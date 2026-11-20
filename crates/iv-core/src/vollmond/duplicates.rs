//! Exact duplicates (IV-17): files with the same content, whatever their names and folders.
//!
//! Candidates come from the index (same size and BLAKE3 hash); the decision is a full byte by
//! byte comparison (point 17). In every group one file is kept — chosen automatically (the earliest
//! date taken, then the fullest metadata) and changeable by the user — and the code refuses to
//! delete a whole group (point 16) or anything in a protected folder (point 19). Deleting goes only
//! to the Recycle Bin through `crate::recycle` (points 1–4), in batches of at most 500 through the
//! journal (point 5); undo brings the files back from the Recycle Bin.

use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use serde::Serialize;

use super::index::Indexed;
use super::plan::{Change, Plan};
use crate::error::{ErrorCode, IvError};
use crate::recycle::{self, Trashed};

/// The tool name in plans and in the journal.
pub const TOOL: &str = "duplicates";

/// A group of files with identical content.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Group {
    pub files: Vec<Indexed>,
    /// The index (in `files`) of the file kept by default.
    pub keep: usize,
}

/// Progress of the search: candidate groups compared of `total`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Progress {
    pub done: usize,
    pub total: usize,
}

/// Whether two files have the same bytes (read in blocks; stops at the first difference).
pub fn same_bytes(a: &Path, b: &Path, cancel: &AtomicBool) -> bool {
    let (Ok(mut x), Ok(mut y)) = (File::open(a), File::open(b)) else {
        return false;
    };
    let mut bx = vec![0u8; 1 << 16];
    let mut by = vec![0u8; 1 << 16];
    loop {
        if cancel.load(Ordering::Relaxed) {
            return false;
        }
        let (Ok(nx), Ok(ny)) = (read_full(&mut x, &mut bx), read_full(&mut y, &mut by)) else {
            return false;
        };
        if nx != ny || bx[..nx] != by[..ny] {
            return false;
        }
        if nx == 0 {
            return true;
        }
    }
}

fn read_full(file: &mut File, buffer: &mut [u8]) -> std::io::Result<usize> {
    let mut filled = 0;
    while filled < buffer.len() {
        match file.read(&mut buffer[filled..])? {
            0 => break,
            n => filled += n,
        }
    }
    Ok(filled)
}

/// How much metadata a file carries (for "keep the original").
fn richness(file: &Indexed) -> usize {
    usize::from(file.taken.is_some())
        + usize::from(file.camera.is_some())
        + usize::from(file.width.is_some())
}

/// The file to keep by default: the earliest date taken (files without one last), then the
/// fullest metadata, then the oldest file date, then the shortest path.
pub fn choose_keep(files: &[Indexed]) -> usize {
    (0..files.len())
        .min_by(|&a, &b| {
            let (x, y) = (&files[a], &files[b]);
            let taken = |f: &Indexed| f.taken.clone().unwrap_or_else(|| "~".into());
            taken(x)
                .cmp(&taken(y))
                .then_with(|| richness(y).cmp(&richness(x)))
                .then_with(|| x.modified.cmp(&y.modified))
                .then_with(|| x.path.len().cmp(&y.path.len()))
                .then_with(|| x.path.cmp(&y.path))
        })
        .unwrap_or(0)
}

/// Find the groups of identical files among `images` (from the index, with hashes).
pub fn find(
    images: &[Indexed],
    cancel: &AtomicBool,
    mut on_progress: impl FnMut(Progress),
) -> Vec<Group> {
    let mut candidates: Vec<&Indexed> = images.iter().filter(|i| i.hash.is_some()).collect();
    candidates.sort_by(|a, b| (a.size, &a.hash).cmp(&(b.size, &b.hash)));
    let buckets: Vec<Vec<&Indexed>> = candidates
        .chunk_by(|a, b| a.size == b.size && a.hash == b.hash)
        .filter(|c| c.len() > 1)
        .map(<[&Indexed]>::to_vec)
        .collect();
    let total = buckets.len();
    let mut groups = Vec::new();
    for (done, bucket) in buckets.into_iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        // Point 17: the hash only proposes; equal bytes decide. A bucket may split.
        let mut rest = bucket;
        while rest.len() > 1 {
            let first = rest[0];
            let (same, other): (Vec<&Indexed>, Vec<&Indexed>) = rest[1..]
                .iter()
                .partition(|f| same_bytes(Path::new(&first.path), Path::new(&f.path), cancel));
            if !same.is_empty() {
                let files: Vec<Indexed> = std::iter::once(first).chain(same).cloned().collect();
                let keep = choose_keep(&files);
                groups.push(Group { files, keep });
            }
            rest = other;
        }
        on_progress(Progress {
            done: done + 1,
            total,
        });
    }
    groups
}

/// Whether `path` lies in one of the protected folders (point 19).
pub fn is_protected(path: &str, protected: &[PathBuf]) -> bool {
    let path = Path::new(path);
    protected.iter().any(|folder| path.starts_with(folder))
}

/// The deletion plan for the user's choice: in each group, `delete` lists the files to remove.
/// Refused when a group would lose every file (point 16) or a protected file is chosen (point 19).
pub fn plan(
    groups: &[Group],
    delete: &[Vec<String>],
    protected: &[PathBuf],
) -> Result<Plan, IvError> {
    let mut changes = Vec::new();
    for (group, chosen) in groups.iter().zip(delete) {
        let members: Vec<&str> = group.files.iter().map(|f| f.path.as_str()).collect();
        if let Some(stranger) = chosen.iter().find(|p| !members.contains(&p.as_str())) {
            return Err(IvError::new(ErrorCode::Other, Path::new(stranger)));
        }
        let left = members
            .iter()
            .filter(|m| !chosen.iter().any(|c| c == *m))
            .count();
        if left == 0 {
            return Err(IvError::new(ErrorCode::WholeGroup, Path::new(members[0])));
        }
        if let Some(locked) = chosen.iter().find(|p| is_protected(p, protected)) {
            return Err(IvError::new(ErrorCode::ProtectedFolder, Path::new(locked)));
        }
        for path in chosen {
            changes.push(Change {
                path: path.clone(),
                field: "trash".into(),
                before: Some("present".into()),
                after: Some("trash".into()),
                suspicious: false,
                note: None,
            });
        }
    }
    Ok(Plan {
        tool: TOOL.into(),
        changes,
    })
}

/// Apply one change: to the Recycle Bin (`after` = "trash"), or back from it (undo, "present").
pub fn apply_change(change: &Change) -> Result<(), IvError> {
    let path = Path::new(&change.path);
    match change.after.as_deref() {
        Some("trash") => recycle::move_to_trash(path).map(|_| ()),
        Some("present") => recycle::restore(&Trashed {
            path: change.path.clone(),
            deleted_at: 0,
        }),
        _ => Err(IvError::new(ErrorCode::Unsupported, path)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vollmond::index::Index;
    use std::fs;

    fn testdata(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../testdata")
            .join(name)
    }

    /// a.jpg = 2026/copy.jpg = other/renamed.JPG; b.png and c.png have the same size, other bytes.
    fn archive() -> tempfile::TempDir {
        let temp = tempfile::tempdir().expect("temp");
        let root = temp.path();
        fs::create_dir_all(root.join("2026")).expect("mkdir");
        fs::create_dir_all(root.join("other")).expect("mkdir");
        fs::copy(testdata("orientation-1.jpg"), root.join("a.jpg")).expect("copy");
        fs::copy(testdata("orientation-1.jpg"), root.join("2026/copy.jpg")).expect("copy");
        fs::copy(
            testdata("orientation-1.jpg"),
            root.join("other/renamed.JPG"),
        )
        .expect("copy");
        let png = fs::read(testdata("red.png")).expect("png");
        fs::write(root.join("b.png"), &png).expect("write");
        let mut other = png.clone();
        let last = other.len() - 20;
        other[last] ^= 0xFF;
        fs::write(root.join("c.png"), other).expect("write");
        temp
    }

    fn indexed(root: &Path) -> Vec<Indexed> {
        let mut index = Index::in_memory().expect("db");
        index
            .index(root, true, true, &AtomicBool::new(false), |_| {})
            .expect("index");
        index.images(root, true).expect("images")
    }

    #[test]
    fn finds_copies_with_other_names_and_folders_only() {
        let temp = archive();
        let groups = find(&indexed(temp.path()), &AtomicBool::new(false), |_| {});
        assert_eq!(
            groups.len(),
            1,
            "same-size PNGs with other bytes are not duplicates"
        );
        let names: Vec<_> = groups[0]
            .files
            .iter()
            .map(|f| f.path.rsplit(['/', '\\']).next().unwrap_or("").to_owned())
            .collect();
        assert_eq!(names.len(), 3);
        assert!(names.contains(&"renamed.JPG".to_owned()));
    }

    #[test]
    fn the_hash_alone_never_decides() {
        let temp = archive();
        let mut images = indexed(temp.path());
        // Pretend b.png and c.png collided in the hash: the byte comparison keeps them apart.
        let hash = images
            .iter()
            .find(|i| i.path.ends_with("b.png"))
            .and_then(|i| i.hash.clone());
        for i in images.iter_mut().filter(|i| i.path.ends_with("c.png")) {
            i.hash = hash.clone();
        }
        let groups = find(&images, &AtomicBool::new(false), |_| {});
        assert!(groups
            .iter()
            .all(|g| g.files.iter().all(|f| !f.path.ends_with(".png"))));
        assert!(
            find(&images, &AtomicBool::new(true), |_| {}).is_empty(),
            "cancel"
        );
    }

    fn file(path: &str, taken: Option<&str>, camera: bool, modified: i64) -> Indexed {
        Indexed {
            path: path.into(),
            size: 1,
            modified,
            taken: taken.map(str::to_owned),
            camera: camera.then(|| "Canon".to_owned()),
            width: None,
            height: None,
            hash: Some("h".into()),
        }
    }

    #[test]
    fn keeps_the_original() {
        let files = [
            file(r"D:\WhatsApp\IMG-1.jpg", None, false, 5),
            file(
                r"D:\Fotos\2019\IMG_1.jpg",
                Some("2019-05-01T10:00:00"),
                true,
                9,
            ),
            file(
                r"D:\Fotos\copy\IMG_1.jpg",
                Some("2019-05-01T10:00:00"),
                false,
                1,
            ),
        ];
        assert_eq!(
            choose_keep(&files),
            1,
            "earliest date taken, then the fullest metadata"
        );
        let no_dates = [file("b.jpg", None, false, 9), file("a.jpg", None, false, 3)];
        assert_eq!(choose_keep(&no_dates), 1, "then the oldest file date");
    }

    #[test]
    fn a_group_never_loses_every_file_and_protected_folders_stay() {
        let group = Group {
            files: vec![
                file("/a/1.jpg", None, false, 1),
                file("/b/2.jpg", None, false, 2),
            ],
            keep: 0,
        };
        let groups = [group];
        let all = vec![vec!["/a/1.jpg".to_owned(), "/b/2.jpg".to_owned()]];
        assert_eq!(
            plan(&groups, &all, &[]).expect_err("all").code,
            ErrorCode::WholeGroup
        );
        let one = vec![vec!["/b/2.jpg".to_owned()]];
        assert_eq!(plan(&groups, &one, &[]).expect("one").changes.len(), 1);
        let protected = [PathBuf::from("/b")];
        assert_eq!(
            plan(&groups, &one, &protected).expect_err("protected").code,
            ErrorCode::ProtectedFolder
        );
        let stranger = vec![vec!["/c/3.jpg".to_owned()]];
        assert!(plan(&groups, &stranger, &[]).is_err());
    }

    /// Delete to the Recycle Bin and undo, on copies (Windows CI: the Recycle Bin exists there).
    #[cfg(windows)]
    #[test]
    fn delete_and_undo_through_the_recycle_bin() {
        use crate::vollmond::plan::{apply, undo_plan, Journal};
        let temp = archive();
        let groups = find(&indexed(temp.path()), &AtomicBool::new(false), |_| {});
        let keep = groups[0].keep;
        let delete: Vec<String> = groups[0]
            .files
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != keep)
            .map(|(_, f)| f.path.clone())
            .collect();
        let plan = plan(&groups, &[delete.clone()], &[]).expect("plan");
        let journal = Journal::new(&temp.path().join("journal"));
        let report = apply(
            &plan,
            0..plan.changes.len(),
            &journal,
            None,
            &AtomicBool::new(false),
            apply_change,
        )
        .expect("apply");
        assert_eq!(report.error, None);
        assert!(delete.iter().all(|p| !Path::new(p).exists()));
        assert!(Path::new(&groups[0].files[keep].path).exists());
        let undo = undo_plan(&journal, report.op).expect("undo");
        apply(
            &undo,
            0..undo.changes.len(),
            &journal,
            Some(report.op),
            &AtomicBool::new(false),
            apply_change,
        )
        .expect("undo");
        assert!(delete.iter().all(|p| Path::new(p).exists()));
    }
}
