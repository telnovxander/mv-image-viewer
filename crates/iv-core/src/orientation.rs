//! Writing a rotation into a JPEG (IV-04), by the safety rules of IV/PROJECT.md, points 6–11:
//!
//! - only the 2 bytes of the existing EXIF Orientation value change; the file is otherwise copied
//!   byte for byte, so pixels and all other metadata (camera, GPS, colour profile, Maker Notes) stay
//!   exactly as they were (points 9 and 11 — checked byte by byte, not assumed);
//! - the plan is shown first ("from → to"), applying is a separate call (point 6);
//! - the journal entry with the old value is written before the file is touched and lives outside
//!   the photo folder (point 7); undo applies the old value the same way;
//! - the new file is written next to the original, read back and checked (Orientation equals the
//!   plan, every other byte equal), and only then renamed over it (points 8 and 10); after the
//!   rename the original is checked once more and put back if anything differs.
//!
//! A JPEG without an Orientation tag would need its EXIF block rebuilt; that is refused here (the
//! metadata writer of IV-14 can add tags).

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::error::{ErrorCode, IvError};
use crate::exif::read_exif;

/// What will change: shown to the user before anything is written.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrientationPlan {
    pub path: String,
    pub from: u16,
    pub to: u16,
}

/// One line of the journal (JSON Lines): written before the change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JournalEntry {
    /// Milliseconds since 1970; also identifies the entry.
    pub id: u64,
    pub path: String,
    pub field: String,
    pub old: u16,
    pub new: u16,
    /// Set on the entry of an undo: the id of the entry it reverses.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub undoes: Option<u64>,
}

pub const JOURNAL_FILE: &str = "journal.jsonl";

/// EXIF Orientation as "mirror horizontally first, then turn clockwise by `degrees`".
fn decompose(code: u16) -> Option<(u16, bool)> {
    Some(match code {
        1 => (0, false),
        2 => (0, true),
        3 => (180, false),
        4 => (180, true),
        5 => (270, true),
        6 => (90, false),
        7 => (90, true),
        8 => (270, false),
        _ => return None,
    })
}

fn compose(degrees: u16, mirrored: bool) -> u16 {
    match (degrees % 360, mirrored) {
        (0, false) => 1,
        (0, true) => 2,
        (180, false) => 3,
        (180, true) => 4,
        (270, true) => 5,
        (90, false) => 6,
        (90, true) => 7,
        _ => 8,
    }
}

/// The Orientation after turning the shown image clockwise by `degrees` (0, 90, 180, 270).
pub fn rotated(code: u16, degrees: u16) -> Option<u16> {
    let (turn, mirrored) = decompose(code)?;
    Some(compose(turn + degrees % 360, mirrored))
}

/// Position of the 2-byte Orientation value inside a JPEG, and whether it is little-endian.
fn orientation_offset(data: &[u8]) -> Option<(usize, bool)> {
    if data.get(0..2)? != [0xFF, 0xD8] {
        return None;
    }
    let mut at = 2;
    // Walk the segments before the image data; EXIF is in an APP1 segment.
    while at + 4 <= data.len() && data[at] == 0xFF {
        let marker = data[at + 1];
        if marker == 0xDA || marker == 0xD9 {
            return None;
        }
        let length = usize::from(u16::from_be_bytes([data[at + 2], data[at + 3]]));
        let body = at + 4;
        if marker == 0xE1 && data.get(body..body + 6) == Some(b"Exif\0\0") {
            return tiff_orientation(data, body + 6, at + 2 + length);
        }
        at += 2 + length;
    }
    None
}

fn tiff_orientation(data: &[u8], tiff: usize, end: usize) -> Option<(usize, bool)> {
    let little = match data.get(tiff..tiff + 2)? {
        b"II" => true,
        b"MM" => false,
        _ => return None,
    };
    let u16_at = |i: usize| -> Option<u16> {
        let b = data.get(i..i + 2)?;
        Some(if little {
            u16::from_le_bytes([b[0], b[1]])
        } else {
            u16::from_be_bytes([b[0], b[1]])
        })
    };
    let u32_at = |i: usize| -> Option<u32> {
        let b = data.get(i..i + 4)?;
        let b = [b[0], b[1], b[2], b[3]];
        Some(if little {
            u32::from_le_bytes(b)
        } else {
            u32::from_be_bytes(b)
        })
    };
    let ifd0 = tiff + usize::try_from(u32_at(tiff + 4)?).ok()?;
    let count = usize::from(u16_at(ifd0)?);
    for i in 0..count {
        let entry = ifd0 + 2 + i * 12;
        if entry + 12 > end {
            return None;
        }
        // Tag 0x0112 Orientation, type SHORT (3), one value: stored in the entry itself.
        if u16_at(entry)? == 0x0112 {
            return (u16_at(entry + 2)? == 3 && u32_at(entry + 4)? == 1)
                .then_some((entry + 8, little));
        }
    }
    None
}

fn value_at(data: &[u8], offset: usize, little: bool) -> Option<u16> {
    let b = data.get(offset..offset + 2)?;
    Some(if little {
        u16::from_le_bytes([b[0], b[1]])
    } else {
        u16::from_be_bytes([b[0], b[1]])
    })
}

fn error(code: ErrorCode, path: &Path) -> IvError {
    IvError::new(code, path)
}

/// The plan for turning the shown image of `path` clockwise by `degrees`. Reads only.
pub fn plan_rotation(path: &Path, degrees: u16) -> Result<OrientationPlan, IvError> {
    let data = fs::read(path).map_err(|e| IvError::from_io(&e, path))?;
    let (offset, little) =
        orientation_offset(&data).ok_or_else(|| error(ErrorCode::NoOrientationTag, path))?;
    let from = value_at(&data, offset, little)
        .filter(|v| (1..=8).contains(v))
        .ok_or_else(|| error(ErrorCode::NoOrientationTag, path))?;
    let to = rotated(from, degrees).ok_or_else(|| error(ErrorCode::NoOrientationTag, path))?;
    Ok(OrientationPlan {
        path: path.display().to_string(),
        from,
        to,
    })
}

/// Apply a plan the user confirmed; the journal goes to `journal_dir`. Returns the journal entry.
pub fn apply(plan: &OrientationPlan, journal_dir: &Path) -> Result<JournalEntry, IvError> {
    write_orientation(plan, journal_dir, None)
}

/// Undo a journal entry: the old value goes back the same safe way, and the undo is journalled.
pub fn undo(entry: &JournalEntry, journal_dir: &Path) -> Result<JournalEntry, IvError> {
    let plan = OrientationPlan {
        path: entry.path.clone(),
        from: entry.new,
        to: entry.old,
    };
    write_orientation(&plan, journal_dir, Some(entry.id))
}

/// All entries of the journal, oldest first (a broken line is skipped, never fatal).
pub fn read_journal(journal_dir: &Path) -> Vec<JournalEntry> {
    fs::read_to_string(journal_dir.join(JOURNAL_FILE))
        .map(|text| {
            text.lines()
                .filter_map(|line| serde_json::from_str(line).ok())
                .collect()
        })
        .unwrap_or_default()
}

fn write_orientation(
    plan: &OrientationPlan,
    journal_dir: &Path,
    undoes: Option<u64>,
) -> Result<JournalEntry, IvError> {
    let path = PathBuf::from(&plan.path);
    let original = fs::read(&path).map_err(|e| IvError::from_io(&e, &path))?;
    let (offset, little) =
        orientation_offset(&original).ok_or_else(|| error(ErrorCode::NoOrientationTag, &path))?;
    // The file must still be as it was when the plan was shown.
    if value_at(&original, offset, little) != Some(plan.from) || !(1..=8).contains(&plan.to) {
        return Err(error(ErrorCode::ChangedMeanwhile, &path));
    }

    // Point 7: the journal entry with the old value, before the file is touched.
    let entry = JournalEntry {
        id: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX)),
        path: plan.path.clone(),
        field: "orientation".into(),
        old: plan.from,
        new: plan.to,
        undoes,
    };
    append_journal(journal_dir, &entry)?;

    let mut changed = original.clone();
    let value = if little {
        plan.to.to_le_bytes()
    } else {
        plan.to.to_be_bytes()
    };
    changed[offset..offset + 2].copy_from_slice(&value);

    // Point 8: a temporary file next to the original, checked, then renamed over it.
    let temp = temp_path(&path);
    let result = write_checked(&temp, &changed, &original, offset, plan.to)
        .and_then(|()| fs::rename(&temp, &path).map_err(|e| IvError::from_io(&e, &path)));
    if result.is_err() {
        let _ = fs::remove_file(&temp);
        return result.map(|()| entry);
    }
    // After the rename: the original must now be exactly the checked file; otherwise put it back.
    if check(&path, &original, offset, plan.to).is_err() {
        let back = temp_path(&path);
        let restored = fs::write(&back, &original)
            .and_then(|()| fs::rename(&back, &path))
            .is_ok();
        let _ = fs::remove_file(&back);
        return Err(error(
            if restored {
                ErrorCode::VerifyFailed
            } else {
                ErrorCode::Other
            },
            &path,
        ));
    }
    Ok(entry)
}

fn temp_path(path: &Path) -> PathBuf {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    path.with_file_name(format!(".{name}.iv-tmp"))
}

fn write_checked(
    temp: &Path,
    changed: &[u8],
    original: &[u8],
    offset: usize,
    to: u16,
) -> Result<(), IvError> {
    let mut file = fs::File::create(temp).map_err(|e| IvError::from_io(&e, temp))?;
    file.write_all(changed)
        .map_err(|e| IvError::from_io(&e, temp))?;
    file.sync_all().map_err(|e| IvError::from_io(&e, temp))?;
    drop(file);
    check(temp, original, offset, to)
}

/// Points 9–11: every byte but the 2 of the Orientation value equals the original, and the
/// Orientation read back by the EXIF reader is the planned one.
fn check(path: &Path, original: &[u8], offset: usize, to: u16) -> Result<(), IvError> {
    let written = fs::read(path).map_err(|e| IvError::from_io(&e, path))?;
    let same_rest = written.len() == original.len()
        && written[..offset] == original[..offset]
        && written[offset + 2..] == original[offset + 2..];
    let read_back = read_exif(path).and_then(|e| e.orientation);
    if same_rest && read_back == Some(to) {
        Ok(())
    } else {
        Err(error(ErrorCode::VerifyFailed, path))
    }
}

fn append_journal(dir: &Path, entry: &JournalEntry) -> Result<(), IvError> {
    fs::create_dir_all(dir).map_err(|e| IvError::from_io(&e, dir))?;
    let path = dir.join(JOURNAL_FILE);
    let line = serde_json::to_string(entry).map_err(|_| error(ErrorCode::Other, &path))?;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| IvError::from_io(&e, &path))?;
    writeln!(file, "{line}").map_err(|e| IvError::from_io(&e, &path))?;
    file.sync_all().map_err(|e| IvError::from_io(&e, &path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exif::tests::{jpeg_with_exif, photo_fields};
    use exif::{Tag, Value};

    /// A JPEG with full EXIF (camera, GPS, …) and some "image data" after it.
    fn photo(orientation: u16) -> Vec<u8> {
        let mut fields = photo_fields();
        fields.retain(|(tag, _)| *tag != Tag::Orientation);
        fields.push((Tag::Orientation, Value::Short(vec![orientation])));
        let mut jpeg = jpeg_with_exif(&fields);
        jpeg.truncate(jpeg.len() - 2);
        // Stand-in for the compressed image: start of scan, data, end of image.
        jpeg.extend_from_slice(&[0xFF, 0xDA, 0x00, 0x02]);
        jpeg.extend((0..4096u32).map(|i| (i * 7 % 251) as u8));
        jpeg.extend_from_slice(&[0xFF, 0xD9]);
        jpeg
    }

    fn setup(orientation: u16) -> (tempfile::TempDir, PathBuf, PathBuf) {
        let temp = tempfile::tempdir().expect("temp dir");
        let photos = temp.path().join("photos");
        fs::create_dir(&photos).expect("photos");
        let path = photos.join("IMG_0001.jpg");
        fs::write(&path, photo(orientation)).expect("write");
        let journal = temp.path().join("journal");
        (temp, path, journal)
    }

    /// Bytes that differ between two files of equal length.
    fn differences(a: &[u8], b: &[u8]) -> Vec<usize> {
        assert_eq!(a.len(), b.len());
        a.iter()
            .zip(b)
            .enumerate()
            .filter(|(_, (x, y))| x != y)
            .map(|(i, _)| i)
            .collect()
    }

    #[test]
    fn rotation_table_turns_every_orientation() {
        for code in 1..=8 {
            assert_eq!(rotated(code, 0), Some(code));
            let four = (0..4).fold(code, |c, _| rotated(c, 90).expect("code"));
            assert_eq!(four, code, "four quarter turns of {code}");
        }
        assert_eq!(rotated(1, 90), Some(6));
        assert_eq!(rotated(6, 90), Some(3));
        assert_eq!(rotated(1, 270), Some(8));
        assert_eq!(rotated(2, 90), Some(7));
        assert_eq!(rotated(9, 90), None);
    }

    #[test]
    fn plan_reads_only() {
        let (_temp, path, _journal) = setup(1);
        let before = fs::read(&path).expect("read");
        let plan = plan_rotation(&path, 90).expect("plan");
        assert_eq!((plan.from, plan.to), (1, 6));
        assert_eq!(fs::read(&path).expect("read"), before);
    }

    #[test]
    fn only_the_orientation_bytes_change() {
        let (_temp, path, journal) = setup(1);
        let before = fs::read(&path).expect("read");
        let exif_before = read_exif(&path).expect("exif");
        apply(&plan_rotation(&path, 90).expect("plan"), &journal).expect("apply");
        let after = fs::read(&path).expect("read");
        // Point 9 and 11: pixels and every other metadata byte are identical.
        let changed = differences(&before, &after);
        assert!(
            changed.len() <= 2 && !changed.is_empty(),
            "changed bytes: {changed:?}"
        );
        let exif_after = read_exif(&path).expect("exif");
        assert_eq!(exif_after.orientation, Some(6));
        assert_eq!(
            without_orientation(exif_after),
            without_orientation(exif_before)
        );
        // Point 8: no temporary file is left.
        let names: Vec<_> = fs::read_dir(path.parent().expect("dir"))
            .expect("read")
            .map(|e| e.expect("entry").file_name())
            .collect();
        assert_eq!(names.len(), 1);
    }

    /// EXIF without the orientation, for comparing everything else.
    fn without_orientation(mut info: crate::ExifInfo) -> crate::ExifInfo {
        info.orientation = None;
        info
    }

    #[test]
    fn journal_first_and_undo_restores_the_file_exactly() {
        let (_temp, path, journal) = setup(3);
        let before = fs::read(&path).expect("read");
        let entry = apply(&plan_rotation(&path, 270).expect("plan"), &journal).expect("apply");
        assert_eq!((entry.old, entry.new), (3, 6));
        // Point 7: the journal is outside the photo folder and survives (it is a file).
        let entries = read_journal(&journal);
        assert_eq!(entries, std::slice::from_ref(&entry));
        assert!(!journal.starts_with(path.parent().expect("dir")));
        let undone = undo(&entry, &journal).expect("undo");
        assert_eq!(undone.undoes, Some(entry.id));
        assert_eq!(fs::read(&path).expect("read"), before);
        assert_eq!(read_journal(&journal).len(), 2);
    }

    #[test]
    fn a_file_changed_after_the_preview_is_left_alone() {
        let (_temp, path, journal) = setup(1);
        let plan = plan_rotation(&path, 90).expect("plan");
        apply(&plan, &journal).expect("first apply");
        let after_first = fs::read(&path).expect("read");
        // The same plan again: the file no longer has "from", so nothing is written.
        let err = apply(&plan, &journal).expect_err("stale plan");
        assert_eq!(err.code, ErrorCode::ChangedMeanwhile);
        assert_eq!(fs::read(&path).expect("read"), after_first);
    }

    #[test]
    fn files_without_the_tag_are_refused_untouched() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = temp.path().join("plain.jpg");
        let jpeg = jpeg_with_exif(&[(Tag::Make, Value::Ascii(vec![b"Canon".to_vec()]))]);
        fs::write(&path, &jpeg).expect("write");
        let err = plan_rotation(&path, 90).expect_err("no tag");
        assert_eq!(err.code, ErrorCode::NoOrientationTag);
        let png = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/red.png");
        let copy = temp.path().join("red.png");
        fs::copy(png, &copy).expect("copy");
        assert_eq!(
            plan_rotation(&copy, 90).expect_err("png").code,
            ErrorCode::NoOrientationTag
        );
        assert_eq!(fs::read(&path).expect("read"), jpeg);
    }

    #[test]
    fn testdata_photo_turned_and_undone_on_a_copy() {
        let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/orientation-6.jpg");
        let temp = tempfile::tempdir().expect("temp dir");
        let path = temp.path().join("copy.jpg");
        fs::copy(&source, &path).expect("copy");
        let before = fs::read(&path).expect("read");
        let entry = apply(
            &plan_rotation(&path, 90).expect("plan"),
            &temp.path().join("j"),
        )
        .expect("apply");
        assert_eq!((entry.old, entry.new), (6, 3));
        assert_eq!(
            differences(&before, &fs::read(&path).expect("read")).len(),
            1
        );
        undo(&entry, &temp.path().join("j")).expect("undo");
        assert_eq!(fs::read(&path).expect("read"), before);
        assert_eq!(
            fs::read(&source).expect("source"),
            before,
            "testdata itself is untouched"
        );
    }

    #[test]
    fn big_endian_exif_is_patched_too() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = temp.path().join("mm.jpg");
        // Build the TIFF part big-endian by hand the TIFF part big-endian by hand: header MM, IFD0 with one entry.
        let tiff: Vec<u8> = [
            &b"MM\0\x2a\0\0\0\x08"[..],
            &[0, 1],
            &[0x01, 0x12, 0, 3, 0, 0, 0, 1, 0, 8, 0, 0],
            &[0, 0, 0, 0],
        ]
        .concat();
        let mut app1 = vec![0xFF, 0xD8, 0xFF, 0xE1];
        app1.extend_from_slice(&u16::try_from(tiff.len() + 8).expect("small").to_be_bytes());
        app1.extend_from_slice(b"Exif\0\0");
        app1.extend_from_slice(&tiff);
        app1.extend_from_slice(&[0xFF, 0xD9]);
        fs::write(&path, &app1).expect("write");
        let plan = plan_rotation(&path, 90).expect("plan");
        assert_eq!((plan.from, plan.to), (8, 1));
        apply(&plan, &temp.path().join("j")).expect("apply");
        assert_eq!(read_exif(&path).and_then(|e| e.orientation), Some(1));
    }
}
