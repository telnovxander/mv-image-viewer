//! The images of a folder and their order (IV-03). The Vollmond index (IV-13) reuses this module.
//!
//! Reading a folder never changes it. Files that vanish or appear between two readings are simply
//! not there or there the next time; nothing here panics on them.

use std::cmp::Ordering;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};

use crate::error::{ErrorCode, IvError};
use crate::formats::format_of;

/// One image of a folder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FolderEntry {
    pub path: String,
    pub name: String,
    pub size: u64,
    /// Modification time, milliseconds since 1970 (0 if unknown).
    pub modified: i64,
    /// Date taken from EXIF (`YYYY-MM-DDTHH:MM:SS`); read only for the "date taken" order.
    pub taken: Option<String>,
}

/// The order of the images.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SortOrder {
    /// Natural order of names: "2" before "10", case does not matter.
    Name,
    Modified,
    /// Date taken from EXIF (IV-04); images without it go last, by name.
    Taken,
}

/// The images Spiegelmond can show in `dir`, sorted. Sub-folders and other files are skipped;
/// an entry that cannot be read (vanished, no access) is skipped too.
pub fn list_images(dir: &Path, order: SortOrder) -> Result<Vec<FolderEntry>, IvError> {
    let read = fs::read_dir(dir).map_err(|e| IvError::from_io(&e, dir))?;
    let mut entries: Vec<FolderEntry> = read
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            format_of(&path)?;
            let metadata = entry.metadata().ok()?;
            if !metadata.is_file() {
                return None;
            }
            let modified = metadata
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX));
            Some(FolderEntry {
                name: entry.file_name().to_string_lossy().into_owned(),
                path: path.display().to_string(),
                size: metadata.len(),
                modified,
                taken: None,
            })
        })
        .collect();
    if order == SortOrder::Taken {
        for entry in &mut entries {
            entry.taken = crate::exif::read_exif(Path::new(&entry.path)).and_then(|e| e.taken);
        }
    }
    sort(&mut entries, order);
    Ok(entries)
}

pub fn sort(entries: &mut [FolderEntry], order: SortOrder) {
    match order {
        SortOrder::Name => entries.sort_by(|a, b| natural_cmp(&a.name, &b.name)),
        SortOrder::Modified => entries.sort_by(|a, b| {
            a.modified
                .cmp(&b.modified)
                .then_with(|| natural_cmp(&a.name, &b.name))
        }),
        // The ISO dates sort as text; images without a date go last.
        SortOrder::Taken => entries.sort_by(|a, b| match (&a.taken, &b.taken) {
            (Some(x), Some(y)) => x.cmp(y).then_with(|| natural_cmp(&a.name, &b.name)),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => natural_cmp(&a.name, &b.name),
        }),
    }
}

/// The folder of a file, for listing its neighbours.
pub fn folder_of(file: &Path) -> Result<PathBuf, IvError> {
    file.parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .ok_or_else(|| IvError::new(ErrorCode::NotFound, file))
}

/// Natural order: runs of digits compare as numbers ("2" < "10"), the rest without case; equal
/// names (e.g. "a01" and "a1") fall back to the plain text so the order is always total.
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    let (mut x, mut y) = (a.chars().peekable(), b.chars().peekable());
    loop {
        match (x.peek().copied(), y.peek().copied()) {
            (None, None) => return a.cmp(b),
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(c), Some(d)) if c.is_ascii_digit() && d.is_ascii_digit() => {
                let left = take_digits(&mut x);
                let right = take_digits(&mut y);
                let (l, r) = (left.trim_start_matches('0'), right.trim_start_matches('0'));
                let ordering = l.len().cmp(&r.len()).then_with(|| l.cmp(r));
                if ordering != Ordering::Equal {
                    return ordering;
                }
            }
            (Some(c), Some(d)) => {
                let ordering = c.to_lowercase().cmp(d.to_lowercase());
                if ordering != Ordering::Equal {
                    return ordering;
                }
                x.next();
                y.next();
            }
        }
    }
}

fn take_digits(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> String {
    let mut digits = String::new();
    while let Some(c) = chars.peek().copied().filter(char::is_ascii_digit) {
        digits.push(c);
        chars.next();
    }
    digits
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, SystemTime};

    fn names(entries: &[FolderEntry]) -> Vec<&str> {
        entries.iter().map(|e| e.name.as_str()).collect()
    }

    /// A temporary folder with copies of the testdata image under the given names.
    fn folder(files: &[&str]) -> tempfile::TempDir {
        let image = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/red.png");
        let temp = tempfile::tempdir().expect("temp dir");
        for name in files {
            fs::copy(&image, temp.path().join(name)).expect("copy");
        }
        temp
    }

    #[test]
    fn natural_order_of_names() {
        let mut list = vec![
            "img10.jpg",
            "IMG2.jpg",
            "img1.jpg",
            "b.jpg",
            "A.jpg",
            "img02.jpg",
        ];
        list.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(
            list,
            [
                "A.jpg",
                "b.jpg",
                "img1.jpg",
                "IMG2.jpg",
                "img02.jpg",
                "img10.jpg"
            ]
        );
        assert_eq!(natural_cmp("x9", "x10"), Ordering::Less);
        assert_eq!(
            natural_cmp("photo 2024-1", "photo 2024-01"),
            "photo 2024-1".cmp("photo 2024-01")
        );
    }

    #[test]
    fn only_supported_files_and_no_folders() {
        let temp = folder(&["a.png", "b.JPG", "c.webp"]);
        fs::write(temp.path().join("notes.txt"), b"text").expect("txt");
        fs::create_dir(temp.path().join("sub.jpg")).expect("dir that looks like an image");
        let list = list_images(temp.path(), SortOrder::Name).expect("list");
        assert_eq!(names(&list), ["a.png", "b.JPG", "c.webp"]);
    }

    #[test]
    fn by_modification_time() {
        let temp = folder(&["new.png", "old.png", "mid.png"]);
        let base = SystemTime::now() - Duration::from_secs(10_000);
        for (name, offset) in [("old.png", 0), ("mid.png", 100), ("new.png", 200)] {
            let file = fs::File::options()
                .write(true)
                .open(temp.path().join(name))
                .expect("open");
            file.set_modified(base + Duration::from_secs(offset))
                .expect("set time");
        }
        let list = list_images(temp.path(), SortOrder::Modified).expect("list");
        assert_eq!(names(&list), ["old.png", "mid.png", "new.png"]);
    }

    #[test]
    fn by_date_taken_and_without_a_date_last() {
        use crate::exif::tests::jpeg_with_exif;
        use exif::{Tag, Value};
        let temp = folder(&["b-no-date.png", "a-no-date.png"]);
        for (name, date) in [
            ("late.jpg", "2024:01:01 10:00:00"),
            ("early.jpg", "2019:07:03 08:15:00"),
        ] {
            let jpeg = jpeg_with_exif(&[(
                Tag::DateTimeOriginal,
                Value::Ascii(vec![date.as_bytes().to_vec()]),
            )]);
            fs::write(temp.path().join(name), jpeg).expect("write");
        }
        let list = list_images(temp.path(), SortOrder::Taken).expect("list");
        assert_eq!(
            names(&list),
            ["early.jpg", "late.jpg", "a-no-date.png", "b-no-date.png"]
        );
        assert_eq!(list[0].taken.as_deref(), Some("2019-07-03T08:15:00"));
    }

    #[test]
    fn a_file_that_vanishes_while_browsing() {
        let temp = folder(&["1.png", "2.png", "3.png"]);
        let first = list_images(temp.path(), SortOrder::Name).expect("list");
        fs::remove_file(temp.path().join("2.png")).expect("remove");
        // The window still holds the old list: opening the vanished file is a clean error…
        let gone = crate::describe(Path::new(&first[1].path)).expect_err("gone");
        assert_eq!(gone.code, ErrorCode::NotFound);
        // …and the next reading has the folder as it is now, a new file included.
        fs::copy(temp.path().join("1.png"), temp.path().join("4.png")).expect("add");
        let again = list_images(temp.path(), SortOrder::Name).expect("list");
        assert_eq!(names(&again), ["1.png", "3.png", "4.png"]);
    }

    #[test]
    fn missing_folder_is_an_error() {
        let temp = tempfile::tempdir().expect("temp dir");
        let err = list_images(&temp.path().join("nope"), SortOrder::Name).expect_err("missing");
        assert_eq!(err.code, ErrorCode::NotFound);
        assert_eq!(
            folder_of(&temp.path().join("a.png")).expect("folder"),
            temp.path()
        );
    }

    #[test]
    fn listing_does_not_change_the_folder() {
        let temp = folder(&["a.png", "b.png"]);
        let before: Vec<_> = fs::read_dir(temp.path())
            .expect("read")
            .map(|e| e.expect("entry").file_name())
            .collect();
        list_images(temp.path(), SortOrder::Name).expect("list");
        let after: Vec<_> = fs::read_dir(temp.path())
            .expect("read")
            .map(|e| e.expect("entry").file_name())
            .collect();
        assert_eq!(before.len(), after.len());
    }

    /// Timing of a large folder: `cargo test -p iv-core --release -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn timing_of_a_large_folder() {
        for count in [1_000, 5_000, 20_000] {
            let temp = tempfile::tempdir().expect("temp dir");
            for i in 0..count {
                fs::write(temp.path().join(format!("IMG_{i:05}.jpg")), b"x").expect("write");
            }
            let start = std::time::Instant::now();
            let list = list_images(temp.path(), SortOrder::Name).expect("list");
            println!("{count} files: {:?}", start.elapsed());
            assert_eq!(list.len(), count);
        }
    }
}
