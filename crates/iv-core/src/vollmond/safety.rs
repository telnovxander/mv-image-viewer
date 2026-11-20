//! The safety rules every Vollmond tool shares (IV/PROJECT.md, "Безопасность (ОБЯЗАТЕЛЬНО)"):
//! batches of at most 500 files (point 5), a trial batch of 10 first for 100+ files (point 14),
//! a warning for folders synced to a cloud or on the network (point 15), and the check that the
//! picture of a file never changes when its metadata does (point 9).

use std::io::Cursor;
use std::ops::Range;
use std::path::Path;

use serde::Serialize;

use crate::error::{ErrorCode, IvError};

/// Point 5: at most this many files in one operation; more go in batches, each confirmed.
pub const MAX_BATCH: usize = 500;
/// Point 14: operations on this many files or more start with a trial batch.
pub const TRIAL_FROM: usize = 100;
/// Point 14: the size of the trial batch.
pub const TRIAL_BATCH: usize = 10;

/// The batches of an operation on `total` files, in order; each needs its own confirmation.
pub fn batches(total: usize) -> Vec<Range<usize>> {
    let mut out = Vec::new();
    let mut start = 0;
    if total >= TRIAL_FROM {
        out.push(0..TRIAL_BATCH);
        start = TRIAL_BATCH;
    }
    while start < total {
        let end = (start + MAX_BATCH).min(total);
        out.push(start..end);
        start = end;
    }
    out
}

/// Point 15: why a folder deserves a warning before anything is applied in it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FolderWarning {
    /// Synced by OneDrive, kDrive, Google Drive, Dropbox, iCloud, …: changes go to every device.
    Cloud,
    /// A network share: slower, and other people may use the files.
    Network,
}

/// Folder names of sync clients (compared without case).
const CLOUD_NAMES: &[&str] = &[
    "onedrive",
    "kdrive",
    "google drive",
    "googledrive",
    "my drive",
    "dropbox",
    "icloud drive",
    "iclouddrive",
    "nextcloud",
    "owncloud",
    "pcloud drive",
    "mega",
    "yandex.disk",
    "yandexdisk",
];

/// Point 15 for `folder`: a network path, or a folder inside a sync client's folder.
pub fn folder_warning(folder: &Path) -> Option<FolderWarning> {
    if crate::recycle::is_network_path(&folder.display().to_string()) {
        return Some(FolderWarning::Network);
    }
    // Split by hand: a Windows path is checked the same way on every system.
    let text = folder.display().to_string();
    let cloud = text.split(['\\', '/']).any(|part| {
        let name = part.to_lowercase();
        CLOUD_NAMES.contains(&name.as_str())
            || name.starts_with("onedrive - ")
            || name.starts_with("kdrive")
    });
    cloud.then_some(FolderWarning::Cloud)
}

/// Point 9: a fingerprint of the picture of an image, independent of its metadata. For JPEG the
/// compressed image data (from the first start of scan to the end), which metadata changes never
/// touch; for other formats the decoded pixels.
pub fn picture_hash(bytes: &[u8], path: &Path) -> Result<String, IvError> {
    if bytes.starts_with(&[0xFF, 0xD8]) {
        if let Some(scan) = jpeg_scan_start(bytes) {
            return Ok(blake3::hash(&bytes[scan..]).to_hex().to_string());
        }
        return Err(IvError::new(ErrorCode::Broken, path));
    }
    let picture = image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .ok()
        .and_then(|r| r.decode().ok())
        .ok_or_else(|| IvError::new(ErrorCode::Broken, path))?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(&picture.width().to_le_bytes());
    hasher.update(&picture.height().to_le_bytes());
    hasher.update(picture.as_bytes());
    Ok(hasher.finalize().to_hex().to_string())
}

/// Where the image data of a JPEG starts: the first SOS marker after the header segments.
fn jpeg_scan_start(bytes: &[u8]) -> Option<usize> {
    let mut at = 2;
    while at + 4 <= bytes.len() {
        if bytes[at] != 0xFF {
            return None;
        }
        let marker = bytes[at + 1];
        if marker == 0xDA {
            return Some(at);
        }
        let length = usize::from(u16::from_be_bytes([bytes[at + 2], bytes[at + 3]]));
        at += 2 + length;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn testdata(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../testdata")
            .join(name)
    }

    #[test]
    fn batches_of_500_with_a_trial_of_10() {
        assert_eq!(batches(0), Vec::<Range<usize>>::new());
        assert_eq!(batches(99), vec![0..99]);
        assert_eq!(batches(100), vec![0..10, 10..100]);
        assert_eq!(batches(1234), vec![0..10, 10..510, 510..1010, 1010..1234]);
        assert!(batches(50_000).iter().all(|b| b.len() <= MAX_BATCH));
        assert_eq!(
            batches(50_000).iter().map(Range::len).sum::<usize>(),
            50_000
        );
    }

    #[test]
    fn cloud_and_network_folders_are_warned_about() {
        for cloud in [
            r"C:\Users\Alex\OneDrive\Bilder",
            r"C:\Users\Alex\OneDrive - Firma\Fotos",
            r"D:\kDrive\Fotos",
            r"C:\Users\Alex\Google Drive\Fotos",
            "/home/alex/Dropbox/Camera Uploads",
        ] {
            assert_eq!(
                folder_warning(Path::new(cloud)),
                Some(FolderWarning::Cloud),
                "{cloud}"
            );
        }
        assert_eq!(
            folder_warning(Path::new(r"\\nas\fotos")),
            Some(FolderWarning::Network)
        );
        assert_eq!(folder_warning(Path::new(r"D:\Fotos\2026")), None);
        assert_eq!(
            folder_warning(Path::new("/home/alex/Pictures/onedrive-export")),
            None
        );
    }

    #[test]
    fn the_picture_hash_ignores_metadata() {
        // orientation-1 and orientation-3 differ in the picture; the same picture with another
        // EXIF block has the same hash.
        let one = std::fs::read(testdata("orientation-1.jpg")).expect("jpeg");
        let three = std::fs::read(testdata("orientation-3.jpg")).expect("jpeg");
        let path = testdata("orientation-1.jpg");
        let retagged = crate::decode::with_orientation(&one, 8);
        assert_eq!(
            picture_hash(&one, &path).expect("a"),
            picture_hash(&retagged, &path).expect("b")
        );
        assert_ne!(
            picture_hash(&one, &path).expect("a"),
            picture_hash(&three, &path).expect("c")
        );
        let png = std::fs::read(testdata("red.png")).expect("png");
        assert_eq!(picture_hash(&png, &path).expect("png").len(), 64);
        let broken = std::fs::read(testdata("broken.jpg")).expect("broken");
        assert!(picture_hash(&broken, &path).is_err());
    }
}
