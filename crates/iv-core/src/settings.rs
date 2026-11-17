//! The settings file `%LOCALAPPDATA%\Spiegelmond\settings.json` (MV/STANDARD.md); the portable
//! build keeps it next to the exe, in `Spiegelmond-data`, when that folder can be written (as in Halbmond).
//!
//! The core stores the JSON text as is; its shape belongs to the interface. Writes are atomic:
//! a temporary file next to the target, then a rename, so a crash never leaves half a file.

use std::fs;
use std::path::{Path, PathBuf};

use crate::error::IvError;

pub const APP_FOLDER: &str = "Spiegelmond";
pub const SETTINGS_FILE: &str = "settings.json";

/// The data folder of the portable build, next to the exe.
pub const PORTABLE_FOLDER: &str = "Spiegelmond-data";
/// The installer puts the uninstaller next to the exe; a copy without it is the portable build.
pub const UNINSTALLER: &str = "uninstall.exe";

/// Where the settings live: next to the exe for the portable build (release builds on Windows),
/// otherwise [`installed_dir`].
pub fn data_dir() -> Option<PathBuf> {
    #[cfg(all(windows, not(debug_assertions)))]
    if let Some(dir) = std::env::current_exe()
        .ok()
        .and_then(|exe| portable_dir(&exe))
    {
        return Some(dir);
    }
    installed_dir()
}

/// `%LOCALAPPDATA%\Spiegelmond` on Windows, the local data folder elsewhere.
pub fn installed_dir() -> Option<PathBuf> {
    dirs::data_local_dir().map(|dir| dir.join(APP_FOLDER))
}

/// `<exe folder>\Spiegelmond-data` for a copy without the uninstaller next to it, if that folder
/// exists or can be created and is writable; `None` for an installed copy or a read-only folder
/// (then the settings go to [`installed_dir`]).
pub fn portable_dir(exe: &Path) -> Option<PathBuf> {
    let folder = exe.parent()?;
    if folder.join(UNINSTALLER).exists() {
        return None;
    }
    let dir = folder.join(PORTABLE_FOLDER);
    if !dir.is_dir() && fs::create_dir(&dir).is_err() {
        return None;
    }
    writable(&dir).then_some(dir)
}

fn writable(dir: &Path) -> bool {
    let probe = dir.join(".write-test");
    match fs::write(&probe, b"") {
        Ok(()) => {
            // Leaving the empty probe behind is harmless, so a failed removal is no error.
            let _ = fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

/// The settings text, or `None` if there is no settings file yet.
pub fn load(dir: &Path) -> Result<Option<String>, IvError> {
    let path = dir.join(SETTINGS_FILE);
    match fs::read_to_string(&path) {
        Ok(text) => Ok(Some(text)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(IvError::from_io(&e, &path)),
    }
}

pub fn save(dir: &Path, text: &str) -> Result<(), IvError> {
    fs::create_dir_all(dir).map_err(|e| IvError::from_io(&e, dir))?;
    let path = dir.join(SETTINGS_FILE);
    let temp = dir.join(format!("{SETTINGS_FILE}.tmp"));
    fs::write(&temp, text).map_err(|e| IvError::from_io(&e, &temp))?;
    fs::rename(&temp, &path).map_err(|e| IvError::from_io(&e, &path))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_is_none_then_saved_text_comes_back() {
        let temp = tempfile::tempdir().expect("temp dir");
        let dir = temp.path().join("Spiegelmond");
        assert_eq!(load(&dir).expect("load"), None);
        save(&dir, r#"{"theme":"dark"}"#).expect("save");
        save(&dir, r#"{"theme":"light"}"#).expect("save again");
        assert_eq!(
            load(&dir).expect("load").as_deref(),
            Some(r#"{"theme":"light"}"#)
        );
        assert!(!dir.join("settings.json.tmp").exists());
    }

    #[test]
    fn portable_copy_keeps_data_next_to_the_exe() {
        let temp = tempfile::tempdir().expect("temp dir");
        let exe = temp.path().join("Spiegelmond-0.1.0-portable.exe");
        fs::write(&exe, b"").expect("exe");
        let dir = portable_dir(&exe).expect("portable dir");
        assert_eq!(dir, temp.path().join(PORTABLE_FOLDER));
        assert!(dir.is_dir());
        assert!(!dir.join(".write-test").exists());
        // The second start finds the folder that is already there.
        assert_eq!(portable_dir(&exe), Some(dir));
    }

    #[test]
    fn installed_copy_is_not_portable() {
        let temp = tempfile::tempdir().expect("temp dir");
        let exe = temp.path().join("iv.exe");
        fs::write(&exe, b"").expect("exe");
        fs::write(temp.path().join(UNINSTALLER), b"").expect("uninstaller");
        assert_eq!(portable_dir(&exe), None);
        assert!(!temp.path().join(PORTABLE_FOLDER).exists());
    }

    #[test]
    fn folder_that_cannot_hold_data_is_not_portable() {
        let temp = tempfile::tempdir().expect("temp dir");
        let exe = temp.path().join("iv.exe");
        fs::write(&exe, b"").expect("exe");
        // A file where the data folder should be: it can be neither used nor created.
        fs::write(temp.path().join(PORTABLE_FOLDER), b"").expect("blocking file");
        assert_eq!(portable_dir(&exe), None);
    }
}
