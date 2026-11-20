//! "Open with" (IV-07): Windows' own dialog to pick a program for a file, as in Explorer.
//! The same call as in Halbmond (FM-20, `fm-core/src/open_with.rs`); elsewhere it is refused.

use std::path::Path;

use crate::error::{ErrorCode, IvError};

/// Show "Open with → Choose another app" for `path`; `owner` is the window handle (0: none).
/// Closing the dialog without a choice is not an error.
#[cfg(windows)]
pub fn choose_program(path: &Path, owner: isize) -> Result<(), IvError> {
    use windows::core::{HSTRING, PCWSTR};
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::Shell::{
        SHOpenWithDialog, OAIF_ALLOW_REGISTRATION, OAIF_EXEC, OPENASINFO,
    };

    let file = HSTRING::from(path.as_os_str());
    let info = OPENASINFO {
        pcszFile: PCWSTR(file.as_ptr()),
        pcszClass: PCWSTR::null(),
        oaifInFlags: OAIF_ALLOW_REGISTRATION | OAIF_EXEC,
    };
    let window = (owner != 0).then_some(HWND(owner as *mut std::ffi::c_void));
    // SAFETY: `info` and the string it points to live during the call.
    match unsafe { SHOpenWithDialog(window, &info) } {
        Ok(()) => Ok(()),
        // ERROR_CANCELLED: the dialog was closed without a choice.
        Err(e) if e.code().0 as u32 == 0x8007_04C7 => Ok(()),
        Err(_) => Err(IvError::new(ErrorCode::OpenWithFailed, path)),
    }
}

#[cfg(not(windows))]
pub fn choose_program(path: &Path, _owner: isize) -> Result<(), IvError> {
    Err(IvError::new(ErrorCode::OpenWithFailed, path))
}
