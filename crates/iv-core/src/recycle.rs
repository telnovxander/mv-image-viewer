//! Deleting to the Recycle Bin and bringing files back (IV-07), shared with the duplicates of
//! Vollmond (IV-17). IV/PROJECT.md, "Безопасность (ОБЯЗАТЕЛЬНО)":
//!
//! 1. Files go **only** to the Recycle Bin; this module has no way to delete permanently.
//! 3. Where the Recycle Bin does not work (removable and network drives, no or disabled bin), the
//!    file is not touched and the error says why — Windows would otherwise delete it for good.
//! 4. [`restore`] brings the last deleted file back from the Recycle Bin.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{ErrorCode, IvError};

/// Where a file lies, as far as the Recycle Bin is concerned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Place {
    /// A local fixed disk: it has a Recycle Bin.
    Fixed,
    /// USB sticks, memory cards: Windows has no Recycle Bin there.
    Removable,
    /// Network shares (`\\server\share`, mapped drives): no Recycle Bin.
    Network,
    /// CD/DVD, RAM disks, unknown: no Recycle Bin.
    Other,
}

/// Whether a file at `place` may be deleted: the error code that refuses it, if any.
/// `has_bin`: the system says this drive has a working Recycle Bin.
pub fn refusal(place: Place, has_bin: bool) -> Option<ErrorCode> {
    match place {
        Place::Removable => Some(ErrorCode::TrashRemovable),
        Place::Network => Some(ErrorCode::TrashNetwork),
        Place::Other => Some(ErrorCode::TrashUnavailable),
        Place::Fixed if !has_bin => Some(ErrorCode::TrashUnavailable),
        Place::Fixed => None,
    }
}

/// `\\server\share\…` (and `\\?\UNC\…`): a network path whatever the drive type says.
pub fn is_network_path(path: &str) -> bool {
    let path = path.replace('/', "\\");
    let upper = path.to_ascii_uppercase();
    upper.starts_with(r"\\?\UNC\") || (path.starts_with(r"\\") && !path.starts_with(r"\\?\"))
}

/// A file moved to the Recycle Bin: what [`restore`] needs to bring it back.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Trashed {
    pub path: String,
    /// When it was deleted (seconds since 1970), to tell it from older copies in the bin.
    pub deleted_at: i64,
}

/// Move the file `path` to the Recycle Bin. Folders are refused (Spiegelmond deletes images);
/// a place without a working Recycle Bin is refused before anything happens (point 3).
pub fn move_to_trash(path: &Path) -> Result<Trashed, IvError> {
    let metadata = std::fs::symlink_metadata(path).map_err(|e| IvError::from_io(&e, path))?;
    if !metadata.is_file() {
        return Err(IvError::new(ErrorCode::NotAFile, path));
    }
    let (place, has_bin) = platform::place_of(path);
    if let Some(code) = refusal(place, has_bin) {
        return Err(IvError::new(code, path));
    }
    let before = now();
    trash::delete(path).map_err(|_| IvError::new(ErrorCode::TrashFailed, path))?;
    if path.exists() {
        return Err(IvError::new(ErrorCode::TrashFailed, path));
    }
    Ok(Trashed {
        path: path.display().to_string(),
        deleted_at: before,
    })
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

/// Bring `trashed` back from the Recycle Bin to where it was. If a file with that name has
/// appeared there meanwhile, nothing is overwritten: the error says so.
#[cfg(any(windows, all(unix, not(target_os = "macos"))))]
pub fn restore(trashed: &Trashed) -> Result<(), IvError> {
    let path = Path::new(&trashed.path);
    if path.exists() {
        return Err(IvError::new(ErrorCode::RestoreOccupied, path));
    }
    let failed = || IvError::new(ErrorCode::RestoreFailed, path);
    let items = trash::os_limited::list().map_err(|_| failed())?;
    // The newest copy of this path deleted not before this deletion (a minute of slack for clocks).
    let item = items
        .into_iter()
        .filter(|item| {
            same_place(&item.original_path(), path) && item.time_deleted >= trashed.deleted_at - 60
        })
        .max_by_key(|item| item.time_deleted)
        .ok_or_else(failed)?;
    trash::os_limited::restore_all([item]).map_err(|_| failed())?;
    if path.is_file() {
        Ok(())
    } else {
        Err(failed())
    }
}

/// Whether two paths name the same file, although it no longer exists: the same name in the same
/// folder. The folders are compared resolved, because Windows writes the long path into the
/// Recycle Bin while the program may know a short (8.3) one, such as `C:\Users\RUNNER~1\…`.
pub fn same_place(a: &Path, b: &Path) -> bool {
    let name = |p: &Path| p.file_name().map(|n| n.to_string_lossy().to_lowercase());
    if name(a).is_none() || name(a) != name(b) {
        return false;
    }
    match (a.parent(), b.parent()) {
        (Some(x), Some(y)) => {
            x == y
                || matches!(
                    (std::fs::canonicalize(x), std::fs::canonicalize(y)),
                    (Ok(x), Ok(y)) if x == y
                )
        }
        _ => false,
    }
}

#[cfg(not(any(windows, all(unix, not(target_os = "macos")))))]
pub fn restore(trashed: &Trashed) -> Result<(), IvError> {
    Err(IvError::new(
        ErrorCode::RestoreFailed,
        Path::new(&trashed.path),
    ))
}

#[cfg(windows)]
mod platform {
    use super::{is_network_path, Place};
    use std::os::windows::ffi::OsStrExt;
    use std::path::Path;
    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::{GetDriveTypeW, GetVolumePathNameW};
    use windows::Win32::UI::Shell::{SHQueryRecycleBinW, SHQUERYRBINFO};

    const DRIVE_REMOVABLE: u32 = 2;
    const DRIVE_FIXED: u32 = 3;
    const DRIVE_REMOTE: u32 = 4;

    fn wide(text: &std::ffi::OsStr) -> Vec<u16> {
        text.encode_wide().chain(Some(0)).collect()
    }

    /// The kind of drive `path` is on and whether it has a Recycle Bin.
    pub fn place_of(path: &Path) -> (Place, bool) {
        if is_network_path(&path.display().to_string()) {
            return (Place::Network, false);
        }
        let file = wide(path.as_os_str());
        let mut root = vec![0u16; 1024];
        // SAFETY: a NUL-terminated path and a buffer for the volume root.
        if unsafe { GetVolumePathNameW(PCWSTR(file.as_ptr()), &mut root) }.is_err() {
            return (Place::Other, false);
        }
        // SAFETY: the NUL-terminated root just written.
        let kind = unsafe { GetDriveTypeW(PCWSTR(root.as_ptr())) };
        let place = match kind {
            DRIVE_FIXED => Place::Fixed,
            DRIVE_REMOVABLE => Place::Removable,
            DRIVE_REMOTE => Place::Network,
            _ => Place::Other,
        };
        let mut info = SHQUERYRBINFO {
            cbSize: u32::try_from(std::mem::size_of::<SHQUERYRBINFO>()).unwrap_or(0),
            ..Default::default()
        };
        // SAFETY: the root of the volume and a sized info struct. It fails where the drive has
        // no Recycle Bin.
        let has_bin = unsafe { SHQueryRecycleBinW(PCWSTR(root.as_ptr()), &mut info) }.is_ok();
        (place, has_bin)
    }
}

#[cfg(not(windows))]
mod platform {
    use super::Place;
    use std::path::Path;

    /// Other systems: the desktop trash of the user (freedesktop.org), Spiegelmond is not
    /// shipped there; local paths count as fixed.
    pub fn place_of(_path: &Path) -> (Place, bool) {
        (Place::Fixed, true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn only_fixed_drives_with_a_recycle_bin() {
        assert_eq!(refusal(Place::Fixed, true), None);
        assert_eq!(
            refusal(Place::Fixed, false),
            Some(ErrorCode::TrashUnavailable)
        );
        assert_eq!(
            refusal(Place::Removable, true),
            Some(ErrorCode::TrashRemovable)
        );
        assert_eq!(refusal(Place::Network, true), Some(ErrorCode::TrashNetwork));
        assert_eq!(
            refusal(Place::Other, true),
            Some(ErrorCode::TrashUnavailable)
        );
    }

    #[test]
    fn network_paths() {
        assert!(is_network_path(r"\\nas\Fotos\a.jpg"));
        assert!(is_network_path(r"\\?\UNC\nas\Fotos\a.jpg"));
        assert!(is_network_path("//nas/Fotos/a.jpg"));
        assert!(!is_network_path(r"C:\Fotos\a.jpg"));
        assert!(!is_network_path(r"\\?\C:\Fotos\a.jpg"));
    }

    #[test]
    fn missing_files_and_folders_are_refused_untouched() {
        let temp = tempfile::tempdir().expect("temp");
        let folder = temp.path().join("folder");
        fs::create_dir(&folder).expect("mkdir");
        fs::write(folder.join("keep.jpg"), b"x").expect("write");
        assert_eq!(
            move_to_trash(&temp.path().join("missing.jpg"))
                .expect_err("missing")
                .code,
            ErrorCode::NotFound
        );
        assert_eq!(
            move_to_trash(&folder).expect_err("folder").code,
            ErrorCode::NotAFile
        );
        assert!(folder.join("keep.jpg").is_file());
    }

    #[test]
    fn same_place_through_another_spelling_of_the_folder() {
        let temp = tempfile::tempdir().expect("temp");
        let folder = temp.path().join("Fotos");
        fs::create_dir(&folder).expect("mkdir");
        let direct = folder.join("a.jpg");
        let roundabout = temp.path().join("Fotos").join(".").join("a.jpg");
        assert!(same_place(&direct, &roundabout));
        assert!(same_place(&direct, &folder.join("A.JPG")));
        assert!(!same_place(&direct, &folder.join("b.jpg")));
        assert!(!same_place(&direct, &temp.path().join("a.jpg")));
    }

    #[test]
    fn restore_never_overwrites() {
        let temp = tempfile::tempdir().expect("temp");
        let path = temp.path().join("back.jpg");
        fs::write(&path, b"new file with the old name").expect("write");
        let trashed = Trashed {
            path: path.display().to_string(),
            deleted_at: 0,
        };
        assert_eq!(
            restore(&trashed).expect_err("occupied").code,
            ErrorCode::RestoreOccupied
        );
        assert_eq!(
            fs::read(&path).expect("read"),
            b"new file with the old name"
        );
    }

    /// A copy of a test image goes to the Recycle Bin and comes back unchanged (Windows CI; on
    /// Linux CI there is no desktop trash to rely on).
    #[cfg(windows)]
    #[test]
    fn delete_to_the_recycle_bin_and_back() {
        let temp = tempfile::tempdir().expect("temp");
        let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/red.png");
        let path = temp
            .path()
            .join(format!("iv-07-{}.png", std::process::id()));
        fs::copy(&source, &path).expect("copy");
        let trashed = move_to_trash(&path).expect("trash");
        assert!(!path.exists());
        restore(&trashed).expect("restore");
        assert_eq!(
            fs::read(&path).expect("read"),
            fs::read(&source).expect("source")
        );
    }
}
