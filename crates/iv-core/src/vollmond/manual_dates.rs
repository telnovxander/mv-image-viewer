//! Setting dates by hand (IV-15): one date for one or many photos, dates with a step (so the order
//! stays), a shift of all dates (a camera in the wrong time zone), or an approximate date ("summer
//! 2005": the middle of the period is written and the journal says it was approximate).
//!
//! Choosing files and a date here is the explicit choice point 12 asks for before an existing date
//! taken is overwritten; the preview marks those changes (`before` is set). Suspicious dates (in
//! the future, before 1990 — old scans) are marked as always and need the extra tick (point 13).
//! The changes are applied by `dates_tool::apply_change` like every dates change.

use serde::Deserialize;

use super::dates::Stamp;
use super::dates_tool::{can_write_date_taken, TOOL};
use super::plan::{Change, Plan};

/// A photo the user chose, with its current dates.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Chosen {
    pub path: String,
    pub taken: Option<Stamp>,
    pub modified: Option<Stamp>,
}

impl<'de> Deserialize<'de> for Stamp {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Stamp::parse(&text).ok_or_else(|| serde::de::Error::custom("not a date"))
    }
}

/// How the new dates are made.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum Manual {
    /// The same date for every file.
    Same { date: Stamp },
    /// `start` for the first file, each next one `step_seconds` later (in the order chosen).
    Step { start: Stamp, step_seconds: i64 },
    /// Every date taken moved by `seconds` (files without one are left out).
    Shift { seconds: i64 },
    /// Only a year, or a year and a month, or a season: the middle of the period is written.
    Approximate {
        year: u16,
        #[serde(default)]
        month: Option<u8>,
        /// 1 spring, 2 summer, 3 autumn, 4 winter (December–February, starting in `year`).
        #[serde(default)]
        season: Option<u8>,
    },
}

/// The middle of a period and its description for the journal.
pub fn middle_of(year: u16, month: Option<u8>, season: Option<u8>) -> Option<(Stamp, String)> {
    let start_end = |y1: u16, m1: u8, y2: u16, m2: u8| -> Option<(i64, i64)> {
        let start = Stamp::new(y1, m1, 1, 0, 0, 0)?.seconds();
        let (ny, nm) = if m2 == 12 { (y2 + 1, 1) } else { (y2, m2 + 1) };
        let end = Stamp::new(ny, nm, 1, 0, 0, 0)?.seconds();
        Some((start, end))
    };
    let ((start, end), label) = match (month, season) {
        (Some(m), _) => (start_end(year, m, year, m)?, format!("{year}-{m:02}")),
        (None, Some(1)) => (start_end(year, 3, year, 5)?, format!("spring {year}")),
        (None, Some(2)) => (start_end(year, 6, year, 8)?, format!("summer {year}")),
        (None, Some(3)) => (start_end(year, 9, year, 11)?, format!("autumn {year}")),
        (None, Some(4)) => (
            start_end(year, 12, year + 1, 2)?,
            format!("winter {year}/{}", year + 1),
        ),
        (None, None) => (start_end(year, 1, year, 12)?, format!("{year}")),
        _ => return None,
    };
    // The middle, rounded to noon of that day: a plain, recognisable time.
    let middle = Stamp::from_seconds((start + end) / 2, 0)?;
    let noon = Stamp::new(middle.year, middle.month, middle.day, 12, 0, 0)?;
    Some((noon, format!("approximate: {label}")))
}

/// The plan for the chosen files. Files whose date taken cannot be written (not JPEG) get only
/// their file date. Every file date follows its new date taken.
pub fn plan(chosen: &[Chosen], how: &Manual, today: Stamp) -> Plan {
    let approximate = match how {
        Manual::Approximate {
            year,
            month,
            season,
        } => middle_of(*year, *month, *season),
        _ => None,
    };
    let mut changes = Vec::new();
    for (i, file) in chosen.iter().enumerate() {
        let new = match how {
            Manual::Same { date } => Some(*date),
            Manual::Step {
                start,
                step_seconds,
            } => {
                let n = i64::try_from(i).unwrap_or(i64::MAX);
                Stamp::from_seconds(start.seconds() + n.saturating_mul(*step_seconds), 0)
            }
            Manual::Shift { seconds } => file
                .taken
                .and_then(|t| Stamp::from_seconds(t.seconds() + seconds, 0)),
            Manual::Approximate { .. } => approximate.as_ref().map(|a| a.0),
        };
        let Some(new) = new else { continue };
        let suspicious = new.year < 1990 || new > today;
        let note = approximate.as_ref().map(|a| a.1.clone());
        if can_write_date_taken(&file.path) && file.taken != Some(new) {
            changes.push(Change {
                path: file.path.clone(),
                field: "taken".into(),
                before: file.taken.map(|t| t.iso()),
                after: Some(new.iso()),
                suspicious,
                note: note.clone(),
            });
        }
        if file.modified != Some(new) {
            changes.push(Change {
                path: file.path.clone(),
                field: "file_modified".into(),
                before: file.modified.map(|m| m.iso()),
                after: Some(new.iso()),
                suspicious,
                note,
            });
        }
    }
    Plan {
        tool: TOOL.into(),
        changes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exif::read_exif;
    use crate::vollmond::dates_tool::apply_change;
    use crate::vollmond::plan::{apply, undo_plan, Journal};
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::AtomicBool;

    fn s(text: &str) -> Stamp {
        Stamp::parse(text).expect(text)
    }

    fn today() -> Stamp {
        s("2026-11-20T12:00:00")
    }

    fn chosen(path: &str, taken: Option<&str>) -> Chosen {
        Chosen {
            path: path.into(),
            taken: taken.map(s),
            modified: Some(today()),
        }
    }

    fn afters(plan: &Plan, field: &str) -> Vec<String> {
        plan.changes
            .iter()
            .filter(|c| c.field == field)
            .filter_map(|c| c.after.clone())
            .collect()
    }

    #[test]
    fn same_step_and_shift() {
        let files = [
            chosen("a.jpg", None),
            chosen("b.jpg", Some("2020-01-01T10:00:00")),
            chosen("c.png", None),
        ];
        let same = plan(
            &files,
            &Manual::Same {
                date: s("2019-06-01T09:00:00"),
            },
            today(),
        );
        assert_eq!(afters(&same, "taken"), ["2019-06-01T09:00:00"; 2]);
        assert_eq!(
            afters(&same, "file_modified").len(),
            3,
            "the PNG gets its file date"
        );
        // Overwriting an existing date taken is explicit and visible in the preview (point 12).
        assert!(same
            .changes
            .iter()
            .any(|c| c.field == "taken" && c.before.is_some()));

        let step = plan(
            &files,
            &Manual::Step {
                start: s("2019-06-01T09:00:00"),
                step_seconds: 60,
            },
            today(),
        );
        assert_eq!(
            afters(&step, "file_modified"),
            [
                "2019-06-01T09:00:00",
                "2019-06-01T09:01:00",
                "2019-06-01T09:02:00"
            ]
        );

        let shift = plan(&files, &Manual::Shift { seconds: -3600 }, today());
        assert_eq!(
            afters(&shift, "taken"),
            ["2020-01-01T09:00:00"],
            "files without a date are left out"
        );
    }

    #[test]
    fn the_middle_of_a_period() {
        assert_eq!(
            middle_of(2005, None, None).map(|m| (m.0.iso(), m.1)),
            Some(("2005-07-02T12:00:00".into(), "approximate: 2005".into()))
        );
        assert_eq!(
            middle_of(2005, Some(2), None).map(|m| m.0.iso()).as_deref(),
            Some("2005-02-15T12:00:00")
        );
        assert_eq!(
            middle_of(2005, None, Some(2)).map(|m| (m.0.iso(), m.1)),
            Some((
                "2005-07-17T12:00:00".into(),
                "approximate: summer 2005".into()
            ))
        );
        assert_eq!(
            middle_of(2005, None, Some(4)).map(|m| m.0.iso()).as_deref(),
            Some("2006-01-15T12:00:00")
        );
        assert_eq!(middle_of(2005, None, Some(7)), None);
        let p = plan(
            &[chosen("a.jpg", None)],
            &Manual::Approximate {
                year: 1987,
                month: None,
                season: Some(2),
            },
            today(),
        );
        assert!(p
            .changes
            .iter()
            .all(|c| c.note.as_deref() == Some("approximate: summer 1987")));
        assert!(
            p.changes.iter().all(|c| c.suspicious),
            "an old scan before 1990 needs the extra tick"
        );
    }

    fn testdata(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../testdata")
            .join(name)
    }

    #[test]
    fn applied_to_copies_and_undone() {
        let temp = tempfile::tempdir().expect("temp");
        let photos = temp.path().join("photos");
        fs::create_dir(&photos).expect("mkdir");
        let paths: Vec<PathBuf> = (0..3)
            .map(|i| {
                let p = photos.join(format!("scan-{i}.jpg"));
                fs::copy(testdata("orientation-1.jpg"), &p).expect("copy");
                p
            })
            .collect();
        let files: Vec<Chosen> = paths
            .iter()
            .map(|p| Chosen {
                path: p.display().to_string(),
                taken: None,
                modified: None,
            })
            .collect();
        let journal = Journal::new(&temp.path().join("journal"));
        let step = plan(
            &files,
            &Manual::Step {
                start: s("2001-02-03T04:05:06"),
                step_seconds: 60,
            },
            today(),
        );
        let report = apply(
            &step,
            0..step.changes.len(),
            &journal,
            None,
            &AtomicBool::new(false),
            apply_change,
        )
        .expect("apply");
        assert_eq!(report.error, None);
        let taken: Vec<_> = paths
            .iter()
            .map(|p| read_exif(p).and_then(|e| e.taken))
            .collect();
        assert_eq!(
            taken,
            [
                Some("2001-02-03T04:05:06".into()),
                Some("2001-02-03T04:06:06".into()),
                Some("2001-02-03T04:07:06".into())
            ]
        );
        // Undo: the dates taken are gone again (file dates without a "before" stay; nothing to go back to).
        let undo = undo_plan(&journal, report.op).expect("undo");
        let undo = Plan {
            changes: undo
                .changes
                .into_iter()
                .filter(|c| c.after.is_some() || c.field == "taken")
                .collect(),
            ..undo
        };
        apply(
            &undo,
            0..undo.changes.len(),
            &journal,
            Some(report.op),
            &AtomicBool::new(false),
            apply_change,
        )
        .expect("undo");
        assert!(paths
            .iter()
            .all(|p| read_exif(p).and_then(|e| e.taken).is_none()));
    }
}
