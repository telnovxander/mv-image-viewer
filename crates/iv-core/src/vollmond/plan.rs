//! The preview engine and the journal of operations shared by every Vollmond tool (IV-13).
//!
//! A tool never changes a file directly. It builds a [`Plan`] — "before → after" per file and
//! field — which the window shows with its totals (point 6). On confirmation [`apply`] works through
//! one batch (points 5 and 14): the batch goes to the journal **before** any file is touched
//! (point 7), then each change is applied by the tool through [`replace_checked`] (atomic, checked,
//! the picture unchanged: points 8–10); the first failure stops the batch. [`undo_plan`] turns a
//! journalled operation into a plan of the reverse changes, applied the same safe way.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use super::safety::{batches, folder_warning, picture_hash, FolderWarning};
use crate::error::{ErrorCode, IvError};

/// The journal of Vollmond operations, next to the journal of IV-04 (outside every photo folder).
pub const OPERATIONS_FILE: &str = "operations.jsonl";

/// One change of one file: shown in the preview, journalled, reversed by undo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Change {
    pub path: String,
    /// What changes, e.g. `taken` (IV-14), `file_modified`, `name`.
    pub field: String,
    pub before: Option<String>,
    pub after: Option<String>,
    /// Point 13: looks wrong (a date in the future, before 1990); not applied unless chosen.
    #[serde(default)]
    pub suspicious: bool,
}

/// What a tool proposes; nothing is changed by building it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Plan {
    pub tool: String,
    pub changes: Vec<Change>,
}

/// The totals the preview shows above the list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PlanSummary {
    pub files: usize,
    pub changes: usize,
    pub suspicious: usize,
    /// The batches it will be applied in, each confirmed (trial of 10 first for 100+ files).
    pub batches: Vec<(usize, usize)>,
    /// Point 15: folders synced to a cloud or on the network.
    pub warnings: Vec<(String, FolderWarning)>,
}

impl Plan {
    pub fn summary(&self) -> PlanSummary {
        let mut files: Vec<&str> = self.changes.iter().map(|c| c.path.as_str()).collect();
        files.sort_unstable();
        files.dedup();
        let mut folders: Vec<PathBuf> = files
            .iter()
            .filter_map(|f| Path::new(f).parent().map(Path::to_path_buf))
            .collect();
        folders.sort();
        folders.dedup();
        PlanSummary {
            files: files.len(),
            changes: self.changes.len(),
            suspicious: self.changes.iter().filter(|c| c.suspicious).count(),
            batches: batches(self.changes.len())
                .into_iter()
                .map(|r| (r.start, r.end))
                .collect(),
            warnings: folders
                .into_iter()
                .filter_map(|f| folder_warning(&f).map(|w| (f.display().to_string(), w)))
                .collect(),
        }
    }
}

/// One line of the journal of operations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum Record {
    /// Written before the first file of a batch is touched: everything needed to go back.
    Started {
        op: u64,
        time: u64,
        tool: String,
        changes: Vec<Change>,
        /// Set when this operation reverses another one.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        undoes: Option<u64>,
    },
    /// After the batch: which changes were applied (by index into `changes`).
    Finished {
        op: u64,
        applied: Vec<usize>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        error: Option<IvError>,
    },
}

/// An operation as the window lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Operation {
    pub op: u64,
    pub time: u64,
    pub tool: String,
    pub undoes: Option<u64>,
    /// The changes that were applied (all of them while an interrupted batch is unknown).
    pub applied: Vec<Change>,
    /// The batch did not finish (the program stopped): its changes may be partly applied.
    pub interrupted: bool,
    pub undone: bool,
}

/// The journal of operations: a JSON Lines file, appended and synced, never rewritten.
pub struct Journal {
    file: PathBuf,
}

impl Journal {
    pub fn new(dir: &Path) -> Self {
        Self {
            file: dir.join(OPERATIONS_FILE),
        }
    }

    fn append(&self, record: &Record) -> Result<(), IvError> {
        if let Some(dir) = self.file.parent() {
            fs::create_dir_all(dir).map_err(|e| IvError::from_io(&e, dir))?;
        }
        let line = serde_json::to_string(record)
            .map_err(|_| IvError::new(ErrorCode::Other, &self.file))?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.file)
            .map_err(|e| IvError::from_io(&e, &self.file))?;
        writeln!(file, "{line}").map_err(|e| IvError::from_io(&e, &self.file))?;
        file.sync_all()
            .map_err(|e| IvError::from_io(&e, &self.file))
    }

    /// All records, oldest first; a broken line (a crash mid-write) is skipped.
    pub fn records(&self) -> Vec<Record> {
        fs::read_to_string(&self.file)
            .map(|text| {
                text.lines()
                    .filter_map(|line| serde_json::from_str(line).ok())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The operations, newest first.
    pub fn operations(&self) -> Vec<Operation> {
        let records = self.records();
        let undone: Vec<u64> = records
            .iter()
            .filter_map(|r| match r {
                Record::Started {
                    undoes: Some(op), ..
                } => Some(*op),
                _ => None,
            })
            .collect();
        let mut list: Vec<Operation> = records
            .iter()
            .filter_map(|record| {
                let Record::Started {
                    op,
                    time,
                    tool,
                    changes,
                    undoes,
                } = record
                else {
                    return None;
                };
                let finished = records.iter().find_map(|r| match r {
                    Record::Finished { op: o, applied, .. } if o == op => Some(applied),
                    _ => None,
                });
                let applied = match finished {
                    Some(indexes) => indexes
                        .iter()
                        .filter_map(|&i| changes.get(i).cloned())
                        .collect(),
                    None => changes.clone(),
                };
                Some(Operation {
                    op: *op,
                    time: *time,
                    tool: tool.clone(),
                    undoes: *undoes,
                    applied,
                    interrupted: finished.is_none(),
                    undone: undone.contains(op),
                })
            })
            .collect();
        list.reverse();
        list
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

/// What applying a batch did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ApplyReport {
    pub op: u64,
    pub applied: usize,
    /// The batch stopped at this change (an error or a cancel); the rest was not touched.
    pub stopped_at: Option<usize>,
    pub error: Option<IvError>,
}

/// Apply the changes `batch` of `plan` with `apply_one` (the tool's own safe write), journal first.
/// Suspicious changes the user did not choose must already be left out of the plan.
pub fn apply(
    plan: &Plan,
    batch: Range<usize>,
    journal: &Journal,
    undoes: Option<u64>,
    cancel: &AtomicBool,
    mut apply_one: impl FnMut(&Change) -> Result<(), IvError>,
) -> Result<ApplyReport, IvError> {
    let changes = plan
        .changes
        .get(batch.clone())
        .ok_or_else(|| IvError::new(ErrorCode::Other, Path::new(&plan.tool)))?
        .to_vec();
    if changes.len() > super::safety::MAX_BATCH {
        return Err(IvError::new(ErrorCode::TooManyFiles, Path::new(&plan.tool)));
    }
    let op = now_ms();
    journal.append(&Record::Started {
        op,
        time: op,
        tool: plan.tool.clone(),
        changes: changes.clone(),
        undoes,
    })?;
    let mut applied = Vec::new();
    let mut error = None;
    let mut stopped_at = None;
    for (i, change) in changes.iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            stopped_at = Some(i);
            break;
        }
        match apply_one(change) {
            Ok(()) => applied.push(i),
            Err(e) => {
                error = Some(e);
                stopped_at = Some(i);
                break;
            }
        }
    }
    journal.append(&Record::Finished {
        op,
        applied: applied.clone(),
        error: error.clone(),
    })?;
    Ok(ApplyReport {
        op,
        applied: applied.len(),
        stopped_at,
        error,
    })
}

/// The plan that reverses operation `op`: every applied change with before and after swapped,
/// last first. Applied with `apply(…, undoes: Some(op), …)`.
pub fn undo_plan(journal: &Journal, op: u64) -> Option<Plan> {
    let operation = journal.operations().into_iter().find(|o| o.op == op)?;
    if operation.undone {
        return None;
    }
    let changes = operation
        .applied
        .into_iter()
        .rev()
        .map(|c| Change {
            path: c.path,
            field: c.field,
            before: c.after,
            after: c.before,
            suspicious: false,
        })
        .collect();
    Some(Plan {
        tool: operation.tool,
        changes,
    })
}

/// Points 8–10: replace the file at `path` with `new_bytes` safely. The new file is written next to
/// it and synced; it must keep the same picture as the original (point 9) and pass the tool's own
/// `verify` (the metadata read back equals the plan, point 10); only then it is renamed over the
/// original. After the rename both checks run once more, and on any difference the original bytes
/// are put back. A crash at any moment leaves either the old or the new whole file.
pub fn replace_checked(
    path: &Path,
    new_bytes: &[u8],
    verify: impl Fn(&Path) -> Result<(), IvError>,
) -> Result<(), IvError> {
    let original = fs::read(path).map_err(|e| IvError::from_io(&e, path))?;
    let picture = picture_hash(&original, path)?;
    let same_picture = |file: &Path| -> Result<(), IvError> {
        let bytes = fs::read(file).map_err(|e| IvError::from_io(&e, file))?;
        if picture_hash(&bytes, file)? == picture {
            Ok(())
        } else {
            Err(IvError::new(ErrorCode::PictureChanged, path))
        }
    };
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let temp = path.with_file_name(format!(".{name}.iv-tmp"));
    let written = (|| {
        let mut file = fs::File::create(&temp).map_err(|e| IvError::from_io(&e, &temp))?;
        file.write_all(new_bytes)
            .map_err(|e| IvError::from_io(&e, &temp))?;
        file.sync_all().map_err(|e| IvError::from_io(&e, &temp))?;
        drop(file);
        same_picture(&temp)?;
        verify(&temp)?;
        fs::rename(&temp, path).map_err(|e| IvError::from_io(&e, path))
    })();
    if let Err(error) = written {
        let _ = fs::remove_file(&temp);
        return Err(error);
    }
    if let Err(error) = same_picture(path).and_then(|()| verify(path)) {
        let back = path.with_file_name(format!(".{name}.iv-back"));
        let restored = fs::write(&back, &original).and_then(|()| fs::rename(&back, path));
        let _ = fs::remove_file(&back);
        return Err(if restored.is_ok() {
            error
        } else {
            IvError::new(ErrorCode::Other, path)
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decode::with_orientation;
    use crate::exif::read_exif;

    fn testdata(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../testdata")
            .join(name)
    }

    /// Copies of testdata photos in a temporary folder, and a journal outside it.
    fn setup(count: usize) -> (tempfile::TempDir, Vec<PathBuf>, Journal) {
        let temp = tempfile::tempdir().expect("temp");
        let photos = temp.path().join("photos");
        fs::create_dir(&photos).expect("mkdir");
        let paths = (0..count)
            .map(|i| {
                let path = photos.join(format!("IMG_{i:03}.jpg"));
                fs::copy(testdata("orientation-1.jpg"), &path).expect("copy");
                path
            })
            .collect();
        let journal = Journal::new(&temp.path().join("data/journal"));
        (temp, paths, journal)
    }

    /// The test tool: writes an EXIF Orientation (a metadata change that keeps the picture).
    fn set_orientation(change: &Change) -> Result<(), IvError> {
        let path = Path::new(&change.path);
        let value: u16 = change
            .after
            .as_deref()
            .and_then(|v| v.parse().ok())
            .unwrap_or(1);
        let bytes = fs::read(path).map_err(|e| IvError::from_io(&e, path))?;
        replace_checked(path, &with_orientation(&bytes, value), |file| {
            if read_exif(file).and_then(|e| e.orientation) == Some(value) {
                Ok(())
            } else {
                Err(IvError::new(ErrorCode::VerifyFailed, file))
            }
        })
    }

    fn plan_for(paths: &[PathBuf], to: &str) -> Plan {
        Plan {
            tool: "test".into(),
            changes: paths
                .iter()
                .map(|p| Change {
                    path: p.display().to_string(),
                    field: "orientation".into(),
                    before: Some("1".into()),
                    after: Some(to.into()),
                    suspicious: false,
                })
                .collect(),
        }
    }

    #[test]
    fn building_a_plan_changes_nothing_and_counts_right() {
        let (_temp, paths, _journal) = setup(3);
        let before: Vec<Vec<u8>> = paths.iter().map(|p| fs::read(p).expect("read")).collect();
        let summary = plan_for(&paths, "6").summary();
        assert_eq!(
            (summary.files, summary.changes, summary.suspicious),
            (3, 3, 0)
        );
        assert_eq!(summary.batches, vec![(0, 3)]);
        assert!(summary.warnings.is_empty());
        for (p, b) in paths.iter().zip(before) {
            assert_eq!(fs::read(p).expect("read"), b);
        }
    }

    #[test]
    fn journal_first_then_apply_then_undo_brings_the_files_back() {
        let (_temp, paths, journal) = setup(3);
        let originals: Vec<Vec<u8>> = paths.iter().map(|p| fs::read(p).expect("read")).collect();
        let plan = plan_for(&paths, "6");
        let report = apply(
            &plan,
            0..3,
            &journal,
            None,
            &AtomicBool::new(false),
            set_orientation,
        )
        .expect("apply");
        assert_eq!((report.applied, report.stopped_at), (3, None));
        for p in &paths {
            assert_eq!(read_exif(p).and_then(|e| e.orientation), Some(6));
        }
        let ops = journal.operations();
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].applied.len(), 3);
        assert!(!ops[0].interrupted && !ops[0].undone);
        let undo = undo_plan(&journal, report.op).expect("undo plan");
        assert_eq!(undo.changes[0].after.as_deref(), Some("1"));
        apply(
            &undo,
            0..3,
            &journal,
            Some(report.op),
            &AtomicBool::new(false),
            set_orientation,
        )
        .expect("undo");
        for p in &paths {
            assert_eq!(read_exif(p).and_then(|e| e.orientation), Some(1));
        }
        // The pictures never changed (point 9).
        for (p, o) in paths.iter().zip(&originals) {
            let now = fs::read(p).expect("read");
            assert_eq!(
                picture_hash(&now, p).expect("a"),
                picture_hash(o, p).expect("b")
            );
        }
        assert!(journal.operations()[1].undone);
        assert!(undo_plan(&journal, report.op).is_none(), "undone once only");
    }

    #[test]
    fn the_first_failure_stops_the_batch_and_the_rest_is_untouched() {
        let (_temp, mut paths, journal) = setup(3);
        paths.insert(1, paths[0].with_file_name("missing.jpg"));
        let plan = plan_for(&paths, "6");
        let report = apply(
            &plan,
            0..4,
            &journal,
            None,
            &AtomicBool::new(false),
            set_orientation,
        )
        .expect("apply");
        assert_eq!((report.applied, report.stopped_at), (1, Some(1)));
        assert_eq!(report.error.map(|e| e.code), Some(ErrorCode::NotFound));
        assert_eq!(read_exif(&paths[2]).and_then(|e| e.orientation), Some(1));
        assert_eq!(journal.operations()[0].applied.len(), 1);
    }

    #[test]
    fn a_change_that_alters_the_picture_is_refused_and_the_file_kept() {
        let (_temp, paths, _journal) = setup(1);
        let original = fs::read(&paths[0]).expect("read");
        let other = fs::read(testdata("orientation-3.jpg")).expect("other");
        let error = replace_checked(&paths[0], &other, |_| Ok(())).expect_err("picture");
        assert_eq!(error.code, ErrorCode::PictureChanged);
        assert_eq!(fs::read(&paths[0]).expect("read"), original);
        let failed_check = replace_checked(&paths[0], &original, |f| {
            Err(IvError::new(ErrorCode::VerifyFailed, f))
        });
        assert_eq!(
            failed_check.expect_err("verify").code,
            ErrorCode::VerifyFailed
        );
        assert_eq!(fs::read(&paths[0]).expect("read"), original);
        let leftovers = fs::read_dir(paths[0].parent().expect("dir"))
            .expect("dir")
            .count();
        assert_eq!(leftovers, 1, "no temporary files left behind");
    }

    #[test]
    fn cancel_and_batch_limits() {
        let (_temp, paths, journal) = setup(2);
        let plan = plan_for(&paths, "6");
        let report = apply(
            &plan,
            0..2,
            &journal,
            None,
            &AtomicBool::new(true),
            set_orientation,
        )
        .expect("apply");
        assert_eq!((report.applied, report.stopped_at), (0, Some(0)));
        let big = Plan {
            tool: "test".into(),
            changes: vec![plan.changes[0].clone(); 501],
        };
        let error = apply(
            &big,
            0..501,
            &journal,
            None,
            &AtomicBool::new(false),
            |_| Ok(()),
        )
        .expect_err("too many");
        assert_eq!(error.code, ErrorCode::TooManyFiles);
        assert_eq!(big.summary().batches[0], (0, 10), "a trial batch first");
    }

    #[test]
    fn an_interrupted_batch_is_shown_as_such() {
        let (_temp, paths, journal) = setup(1);
        journal
            .append(&Record::Started {
                op: 1,
                time: 1,
                tool: "test".into(),
                changes: plan_for(&paths, "6").changes,
                undoes: None,
            })
            .expect("append");
        // A broken last line (a crash while writing) is skipped.
        let mut file = OpenOptions::new()
            .append(true)
            .open(journal.file.clone())
            .expect("open");
        write!(file, "{{\"event\":\"fini").expect("write");
        let ops = journal.operations();
        assert_eq!(ops.len(), 1);
        assert!(ops[0].interrupted);
    }
}
