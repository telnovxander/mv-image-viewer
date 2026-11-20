//! The dates tool (IV-14) put together: rows from the index, the plan, and applying one change
//! through the shared safe path of IV-13 (`plan::apply`, `plan::replace_checked`).

use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::dates::{self, DateRow, Stamp};
use super::exif_write::set_date_taken;
use super::index::Indexed;
use super::plan::{replace_checked, Change, Plan};
use crate::error::{ErrorCode, IvError};
use crate::exif::read_exif;

/// The tool name in plans and in the journal.
pub const TOOL: &str = "dates";

/// Extensions whose date taken this tool can write (JPEG; other formats get only the file time).
pub fn can_write_date_taken(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    [".jpg", ".jpeg", ".jfif"]
        .iter()
        .any(|e| lower.ends_with(e))
        || super::video::is_video(Path::new(path))
}

/// The date taken of a photo (EXIF) or a video (QuickTime, as local time), `YYYY-MM-DDTHH:MM:SS`.
fn taken_of(path: &Path) -> Option<String> {
    if super::video::is_video(path) {
        super::video::file_creation_time(path)
            .and_then(utc_to_local)
            .map(|s| s.iso())
    } else {
        read_exif(path).and_then(|e| e.taken)
    }
}

/// Seconds since 1970 (UTC) of a local wall-clock time, with the zone rules of the system
/// (Windows: daylight saving included). Elsewhere the program is not shipped: UTC.
pub fn local_to_utc(stamp: Stamp) -> i64 {
    platform::local_to_utc(stamp)
}

/// The local wall-clock time of seconds since 1970 (UTC).
pub fn utc_to_local(seconds: i64) -> Option<Stamp> {
    platform::utc_to_local(seconds)
}

/// The rows of the dates table for the images of the index.
pub fn rows(images: &[Indexed], today: Stamp) -> Vec<DateRow> {
    images
        .iter()
        .map(|image| {
            let taken = image.taken.as_deref().and_then(Stamp::parse);
            let modified = utc_to_local(image.modified.div_euclid(1000));
            dates::row(&image.path, taken, modified, today)
        })
        .collect()
}

/// The plan for `rows`: as `dates::plan`, without date-taken changes for files this tool cannot
/// write into (they keep only the file time change). Suspicious changes stay only when
/// `with_suspicious` (point 13: chosen by the user).
pub fn plan(rows: &[DateRow], with_suspicious: bool) -> Plan {
    let mut plan = dates::plan(rows);
    plan.changes.retain(|c| {
        (c.field != "taken" || can_write_date_taken(&c.path)) && (with_suspicious || !c.suspicious)
    });
    plan
}

/// "Prepare for the phone" (IV-16): what in a folder would end up in the wrong place in a phone
/// gallery, and what fixing can do about it. Android takes the date taken from EXIF
/// `DateTimeOriginal` (videos: their creation time); without it, galleries fall back to the file
/// date or to the day the file was added — which copying resets.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct TransferReport {
    pub total: usize,
    /// A date taken inside the file and a matching file date: nothing to do.
    pub ready: usize,
    /// No date taken inside yet, but one from the name can be written (JPEG, video).
    pub fixable_taken: usize,
    /// The file date is off; it will be set to the date taken.
    pub fixable_file_date: usize,
    /// No date inside and the format cannot hold one (PNG, BMP, …): the gallery has only the file
    /// date, which a copy may reset. Copy them so that file dates survive, or convert.
    pub file_date_only: Vec<String>,
    /// No date anywhere (not in EXIF, not in the name): they will sort by the day of copying.
    pub no_date: Vec<String>,
    /// Suspicious dates (future, before 1990), left out unless chosen.
    pub suspicious: usize,
}

pub fn transfer_report(rows: &[DateRow]) -> TransferReport {
    let mut report = TransferReport {
        total: rows.len(),
        ..Default::default()
    };
    for r in rows {
        let writable = can_write_date_taken(&r.path);
        if r.suspicious {
            report.suspicious += 1;
        }
        match (r.taken, r.proposed) {
            (_, None) => report.no_date.push(r.path.clone()),
            (None, Some(_)) if writable => report.fixable_taken += 1,
            (None, Some(_)) => report.file_date_only.push(r.path.clone()),
            (Some(_), Some(_)) if !r.mismatch => report.ready += 1,
            (Some(_), Some(_)) => {}
        }
        if r.proposed.is_some() && r.mismatch && !r.suspicious {
            report.fixable_file_date += 1;
        }
    }
    report
}

fn file_time(path: &Path) -> Result<SystemTime, IvError> {
    fs::metadata(path)
        .and_then(|m| m.modified())
        .map_err(|e| IvError::from_io(&e, path))
}

fn set_file_time(path: &Path, time: SystemTime) -> Result<(), IvError> {
    fs::OpenOptions::new()
        .write(true)
        .open(path)
        .and_then(|f| f.set_modified(time))
        .map_err(|e| IvError::from_io(&e, path))
}

fn system_time(seconds: i64) -> SystemTime {
    match u64::try_from(seconds) {
        Ok(s) => UNIX_EPOCH + Duration::from_secs(s),
        Err(_) => UNIX_EPOCH - Duration::from_secs(seconds.unsigned_abs()),
    }
}

/// The current dates of a file, read from the disk (IV-15: the preview never trusts the window).
pub fn current(path: &Path) -> Result<super::manual_dates::Chosen, IvError> {
    let time = file_time(path)?;
    let seconds = time
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX));
    Ok(super::manual_dates::Chosen {
        path: path.display().to_string(),
        taken: taken_of(path).as_deref().and_then(Stamp::parse),
        modified: utc_to_local(seconds),
    })
}

/// Apply one change of a dates plan (the `apply_one` of `plan::apply`).
///
/// - `taken`: the JPEG gets `DateTimeOriginal` = `after` (or loses it when `after` is empty, for
///   undo), written append-only, checked (date read back, picture the same) and renamed into
///   place; its file time is kept as it was.
/// - `file_modified`: the file time becomes `after` (local time), checked by reading it back.
pub fn apply_change(change: &Change) -> Result<(), IvError> {
    let path = Path::new(&change.path);
    match change.field.as_str() {
        "taken" => {
            if !can_write_date_taken(&change.path) {
                return Err(IvError::new(ErrorCode::Unsupported, path));
            }
            if super::video::is_video(path) {
                if taken_of(path) != change.before {
                    return Err(IvError::new(ErrorCode::ChangedMeanwhile, path));
                }
                let seconds = change
                    .after
                    .as_deref()
                    .and_then(Stamp::parse)
                    .map(local_to_utc);
                return super::video::set_file_creation_time(path, seconds);
            }
            let kept_time = file_time(path)?;
            let bytes = fs::read(path).map_err(|e| IvError::from_io(&e, path))?;
            // The file must still be as the preview showed it (no date where none was).
            let now = read_exif(path).and_then(|e| e.taken);
            if now != change.before {
                return Err(IvError::new(ErrorCode::ChangedMeanwhile, path));
            }
            let new = set_date_taken(&bytes, change.after.as_deref())
                .ok_or_else(|| IvError::new(ErrorCode::Unsupported, path))?;
            let expected = change.after.clone();
            replace_checked(path, &new, |file| {
                if read_exif(file).and_then(|e| e.taken) == expected {
                    Ok(())
                } else {
                    Err(IvError::new(ErrorCode::VerifyFailed, file))
                }
            })?;
            set_file_time(path, kept_time)
        }
        // Undoing a file date that had no readable value before: nothing to go back to.
        "file_modified" if change.after.is_none() => Ok(()),
        "file_modified" => {
            let target = change
                .after
                .as_deref()
                .and_then(Stamp::parse)
                .ok_or_else(|| IvError::new(ErrorCode::Other, path))?;
            let seconds = local_to_utc(target);
            set_file_time(path, system_time(seconds))?;
            let back = file_time(path)?
                .duration_since(UNIX_EPOCH)
                .map_or(i64::MIN, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX));
            if back == seconds {
                Ok(())
            } else {
                Err(IvError::new(ErrorCode::VerifyFailed, path))
            }
        }
        _ => Err(IvError::new(ErrorCode::Unsupported, path)),
    }
}

#[cfg(windows)]
mod platform {
    use super::Stamp;
    use windows::Win32::Foundation::{FILETIME, SYSTEMTIME};
    use windows::Win32::System::Time::{
        FileTimeToSystemTime, SystemTimeToFileTime, SystemTimeToTzSpecificLocalTime,
        TzSpecificLocalTimeToSystemTime,
    };

    /// 100 ns intervals between 1601 and 1970.
    const EPOCH_DIFF: i64 = 11_644_473_600;

    fn to_systemtime(s: Stamp) -> SYSTEMTIME {
        SYSTEMTIME {
            wYear: s.year,
            wMonth: u16::from(s.month),
            wDay: u16::from(s.day),
            wHour: u16::from(s.hour),
            wMinute: u16::from(s.minute),
            wSecond: u16::from(s.second),
            ..Default::default()
        }
    }

    pub fn local_to_utc(stamp: Stamp) -> i64 {
        let local = to_systemtime(stamp);
        let mut utc = SYSTEMTIME::default();
        let mut file = FILETIME::default();
        // SAFETY: valid structs; NULL time zone = the current one, with its DST rules.
        let ok = unsafe {
            TzSpecificLocalTimeToSystemTime(None, &local, &mut utc).is_ok()
                && SystemTimeToFileTime(&utc, &mut file).is_ok()
        };
        if !ok {
            return stamp.seconds();
        }
        let ticks = (i64::from(file.dwHighDateTime) << 32) | i64::from(file.dwLowDateTime);
        ticks / 10_000_000 - EPOCH_DIFF
    }

    pub fn utc_to_local(seconds: i64) -> Option<Stamp> {
        let ticks = (seconds + EPOCH_DIFF).checked_mul(10_000_000)?;
        let file = FILETIME {
            dwLowDateTime: (ticks & 0xFFFF_FFFF) as u32,
            dwHighDateTime: (ticks >> 32) as u32,
        };
        let mut utc = SYSTEMTIME::default();
        let mut local = SYSTEMTIME::default();
        // SAFETY: valid structs; NULL time zone = the current one.
        let ok = unsafe {
            FileTimeToSystemTime(&file, &mut utc).is_ok()
                && SystemTimeToTzSpecificLocalTime(None, &utc, &mut local).is_ok()
        };
        if !ok {
            return Stamp::from_seconds(seconds, 0);
        }
        Stamp::new(
            local.wYear,
            u8::try_from(local.wMonth).ok()?,
            u8::try_from(local.wDay).ok()?,
            u8::try_from(local.wHour).ok()?,
            u8::try_from(local.wMinute).ok()?,
            u8::try_from(local.wSecond).ok()?,
        )
    }
}

#[cfg(not(windows))]
mod platform {
    use super::Stamp;

    pub fn local_to_utc(stamp: Stamp) -> i64 {
        stamp.seconds()
    }

    pub fn utc_to_local(seconds: i64) -> Option<Stamp> {
        Stamp::from_seconds(seconds, 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vollmond::index::Index;
    use crate::vollmond::plan::{apply, undo_plan, Journal};
    use crate::vollmond::safety::picture_hash;
    use std::path::PathBuf;
    use std::sync::atomic::AtomicBool;

    fn testdata(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../testdata")
            .join(name)
    }

    /// A folder of copies, as a phone copy leaves them: today's file time everywhere.
    fn folder() -> (tempfile::TempDir, PathBuf) {
        let temp = tempfile::tempdir().expect("temp");
        let photos = temp.path().join("photos");
        fs::create_dir(&photos).expect("mkdir");
        // WhatsApp: no EXIF at all.
        fs::copy(
            testdata("source/f-1.jpg"),
            photos.join("IMG-20230512-WA0001.jpg"),
        )
        .expect("copy");
        // A camera name and no date taken in EXIF.
        fs::copy(
            testdata("orientation-6.jpg"),
            photos.join("IMG_20220101_101010.jpg"),
        )
        .expect("copy");
        // A PNG screenshot: only the file time can be set.
        fs::copy(
            testdata("red.png"),
            photos.join("Screenshot_20210304_050607.png"),
        )
        .expect("copy");
        // A video (IV-16): the QuickTime date, copied to the phone: file time is today.
        fs::copy(
            testdata("sample.mov"),
            photos.join("VID_20230512_143000.mov"),
        )
        .expect("copy");
        // No date anywhere.
        fs::copy(testdata("blue.bmp"), photos.join("holiday.bmp")).expect("copy");
        (temp, photos)
    }

    fn today() -> Stamp {
        Stamp::parse("2026-11-20T12:00:00").expect("today")
    }

    fn table(photos: &Path) -> Vec<DateRow> {
        let mut index = Index::in_memory().expect("db");
        index
            .index(photos, false, false, &AtomicBool::new(false), |_| {})
            .expect("index");
        rows(&index.images(photos, false).expect("images"), today())
    }

    #[test]
    fn fixes_dates_and_undo_brings_everything_back() {
        let (temp, photos) = folder();
        let before_bytes: Vec<(PathBuf, Vec<u8>, SystemTime)> = fs::read_dir(&photos)
            .expect("dir")
            .map(|e| {
                let p = e.expect("entry").path();
                (
                    p.clone(),
                    fs::read(&p).expect("read"),
                    file_time(&p).expect("time"),
                )
            })
            .collect();
        let plan = plan(&table(&photos), false);
        let fields: Vec<(String, &str)> = plan
            .changes
            .iter()
            .map(|c| {
                let name = Path::new(&c.path)
                    .file_name()
                    .expect("name")
                    .to_string_lossy()
                    .into_owned();
                (name, c.field.as_str())
            })
            .collect();
        assert!(fields.contains(&("IMG-20230512-WA0001.jpg".into(), "taken")));
        assert!(fields.contains(&("IMG_20220101_101010.jpg".into(), "taken")));
        assert!(fields.contains(&("Screenshot_20210304_050607.png".into(), "file_modified")));
        assert!(
            !fields
                .iter()
                .any(|(n, f)| n.ends_with(".png") && *f == "taken"),
            "PNG: file time only"
        );
        assert!(
            !fields.iter().any(|(n, _)| n == "holiday.bmp"),
            "no date anywhere: untouched"
        );

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
        assert_eq!((report.applied, report.error), (plan.changes.len(), None));
        let wa = photos.join("IMG-20230512-WA0001.jpg");
        assert_eq!(
            read_exif(&wa).and_then(|e| e.taken).as_deref(),
            Some("2023-05-12T12:00:00")
        );
        assert_eq!(
            utc_to_local(
                file_time(&wa)
                    .expect("t")
                    .duration_since(UNIX_EPOCH)
                    .expect("d")
                    .as_secs() as i64
            )
            .map(|s| s.iso())
            .as_deref(),
            Some("2023-05-12T12:00:00")
        );
        let shot = photos.join("Screenshot_20210304_050607.png");
        assert_eq!(
            utc_to_local(
                file_time(&shot)
                    .expect("t")
                    .duration_since(UNIX_EPOCH)
                    .expect("d")
                    .as_secs() as i64
            )
            .map(|s| s.iso())
            .as_deref(),
            Some("2021-03-04T05:06:07")
        );
        // The pictures are the same (point 9), the orientation of the camera file is kept (point 11).
        // For the video, its media data (point 9 for videos).
        for (path, bytes, _) in &before_bytes {
            let now = fs::read(path).expect("read");
            if crate::vollmond::video::is_video(path) {
                let hash = crate::vollmond::video::media_hash;
                assert_eq!(hash(&now), hash(bytes));
                continue;
            }
            assert_eq!(
                picture_hash(&now, path).expect("a"),
                picture_hash(bytes, path).expect("b")
            );
        }
        let video = photos.join("VID_20230512_143000.mov");
        assert_eq!(
            utc_to_local(
                file_time(&video)
                    .expect("t")
                    .duration_since(UNIX_EPOCH)
                    .expect("d")
                    .as_secs() as i64
            ),
            crate::vollmond::video::file_creation_time(&video).and_then(utc_to_local),
            "the file date of the video follows its QuickTime date"
        );
        assert_eq!(
            read_exif(&photos.join("IMG_20220101_101010.jpg")).and_then(|e| e.orientation),
            Some(6)
        );
        // After fixing, the table has no mismatches left.
        assert!(table(&photos)
            .iter()
            .all(|r| !r.mismatch || r.proposed.is_none()));

        // Undo: every file as before, file times included.
        let undo = undo_plan(&journal, report.op).expect("undo plan");
        let undone = apply(
            &undo,
            0..undo.changes.len(),
            &journal,
            Some(report.op),
            &AtomicBool::new(false),
            apply_change,
        )
        .expect("undo");
        assert_eq!(undone.error, None);
        for (path, bytes, time) in &before_bytes {
            assert_eq!(
                read_exif(path).and_then(|e| e.taken),
                read_exif_bytes(bytes),
                "{}",
                path.display()
            );
            let secs = |t: &SystemTime| t.duration_since(UNIX_EPOCH).expect("d").as_secs();
            assert_eq!(
                secs(&file_time(path).expect("t")),
                secs(time),
                "{}",
                path.display()
            );
        }
    }

    fn read_exif_bytes(bytes: &[u8]) -> Option<String> {
        let temp = tempfile::tempdir().expect("temp");
        let p = temp.path().join("x.jpg");
        fs::write(&p, bytes).expect("write");
        read_exif(&p).and_then(|e| e.taken)
    }

    #[test]
    fn the_report_before_the_phone() {
        let (_temp, photos) = folder();
        let report = transfer_report(&table(&photos));
        assert_eq!(report.total, 5);
        assert_eq!(
            report.fixable_taken, 2,
            "the WhatsApp JPEG and the camera JPEG"
        );
        assert_eq!(report.file_date_only.len(), 1, "the PNG screenshot");
        assert_eq!(report.no_date.len(), 1, "holiday.bmp");
        assert_eq!(report.ready, 0, "every file was copied today");
        assert_eq!(
            report.fixable_file_date, 4,
            "both JPEGs, the PNG and the video: file date off"
        );
    }

    #[test]
    fn suspicious_dates_are_left_out_unless_chosen() {
        let (_temp, photos) = folder();
        fs::copy(testdata("red.png"), photos.join("IMG_20290101_100000.png")).expect("future");
        let rows = table(&photos);
        assert!(!plan(&rows, false)
            .changes
            .iter()
            .any(|c| c.path.contains("2029")));
        assert!(plan(&rows, true)
            .changes
            .iter()
            .any(|c| c.path.contains("2029") && c.suspicious));
    }

    #[test]
    fn a_file_changed_after_the_preview_is_left_alone() {
        let (_temp, photos) = folder();
        let path = photos.join("IMG-20230512-WA0001.jpg");
        let change = Change {
            path: path.display().to_string(),
            field: "taken".into(),
            before: Some("2020-01-01T00:00:00".into()),
            after: Some("2023-05-12T12:00:00".into()),
            suspicious: false,
            note: None,
        };
        let bytes = fs::read(&path).expect("read");
        assert_eq!(
            apply_change(&change).expect_err("changed").code,
            ErrorCode::ChangedMeanwhile
        );
        assert_eq!(fs::read(&path).expect("read"), bytes);
    }

    /// 50 000 rows (IV-14): `cargo test -p iv-core --release -- --ignored --nocapture dates_of_50_000`.
    #[test]
    #[ignore]
    fn dates_of_50_000() {
        let images: Vec<Indexed> = (0..50_000u32)
            .map(|i| Indexed {
                path: format!(
                    r"D:\Fotos\{:03}\IMG_2023{:02}{:02}_1{:05}.jpg",
                    i / 500,
                    1 + i % 12,
                    1 + i % 28,
                    i % 60_000
                ),
                size: 3_000_000,
                modified: 1_763_600_000_000,
                taken: (i % 3 != 0).then(|| "2023-05-12T14:30:00".to_owned()),
                camera: None,
                width: None,
                height: None,
                hash: None,
            })
            .collect();
        let start = std::time::Instant::now();
        let table = rows(&images, today());
        let built = start.elapsed();
        let json = serde_json::to_vec(&table).expect("json");
        let serialized = start.elapsed();
        let plan = plan(&table, false);
        println!(
            "rows {built:?}, + json {serialized:?} ({} MB), plan {:?}: {} changes, {} batches",
            json.len() / 1_000_000,
            start.elapsed(),
            plan.changes.len(),
            plan.summary().batches.len()
        );
        assert_eq!(table.len(), 50_000);
    }

    #[test]
    fn local_time_round_trip() {
        let s = Stamp::parse("2023-07-01T12:34:56").expect("s");
        assert_eq!(utc_to_local(local_to_utc(s)), Some(s));
    }
}
