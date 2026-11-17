//! Image logic of Spiegelmond, without Tauri or GUI: builds and tests on any OS.
//!
//! IV-01 has what opening one file needs: which files are images, the path from the command line
//! and the description of the file to show; IV-11 adds the settings file, IV-03 the images of a folder. Folder listing, EXIF and decoding come with IV-03…IV-05.

pub mod error;
pub mod folder;
pub mod formats;
pub mod open;
pub mod settings;

pub use error::{ErrorCode, IvError};
pub use folder::{list_images, FolderEntry, SortOrder};
pub use formats::{format_of, Format};
pub use open::{describe, path_from_args, ImageFile};
