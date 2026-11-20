//! The single error type of Spiegelmond.
//!
//! An error carries a stable code and the path, never a ready user text: the interface
//! translates `error.<code>` from its locale files (MV/I18N.md, "Backend errors").

use std::io;
use std::path::Path;

use serde::Serialize;

/// Stable error codes shared with the interface (`error.<code>` keys).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    NotFound,
    AccessDenied,
    NotAFile,
    Unsupported,
    /// Writing a rotation: the JPEG has no Orientation tag to change in place.
    NoOrientationTag,
    /// The file changed between the preview and applying.
    ChangedMeanwhile,
    /// The written file did not match the plan; the original was kept or put back.
    VerifyFailed,
    /// The file is damaged or not what its extension says; the window shows a placeholder.
    Broken,
    /// HEIC/HEIF: the decoder libraries (libheif, libde265) are missing next to the program.
    NoCodec,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IvError {
    pub code: ErrorCode,
    pub path: String,
}

impl IvError {
    pub fn new(code: ErrorCode, path: &Path) -> Self {
        Self {
            code,
            path: path.display().to_string(),
        }
    }

    pub fn from_io(error: &io::Error, path: &Path) -> Self {
        let code = match error.kind() {
            io::ErrorKind::NotFound => ErrorCode::NotFound,
            io::ErrorKind::PermissionDenied => ErrorCode::AccessDenied,
            _ => ErrorCode::Other,
        };
        Self::new(code, path)
    }
}

impl std::fmt::Display for IvError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {}", self.code, self.path)
    }
}

impl std::error::Error for IvError {}
