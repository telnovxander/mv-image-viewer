//! Opening one file: the path from the command line and the description of the file to show.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::error::{ErrorCode, IvError};
use crate::formats::{format_of, Format};

/// A file the window can show.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ImageFile {
    /// Absolute path, as the asset protocol needs it.
    pub path: String,
    /// File name for the title.
    pub name: String,
    pub size: u64,
    pub format: Format,
}

/// The file to open from the arguments of `iv` (the first one is the program itself): the first
/// argument that is not an option, relative to `cwd`. `iv C:\photos\a.jpg`, a double-click on a
/// file and a second start (single instance) all come here.
pub fn path_from_args(args: &[String], cwd: &Path) -> Option<PathBuf> {
    let arg = args.iter().skip(1).find(|a| !a.starts_with('-'))?;
    let path = PathBuf::from(arg);
    Some(if path.is_absolute() {
        path
    } else {
        cwd.join(path)
    })
}

/// Check that `path` is an existing image file Spiegelmond can show, and describe it.
/// Never changes the file.
pub fn describe(path: &Path) -> Result<ImageFile, IvError> {
    let metadata = fs::metadata(path).map_err(|e| IvError::from_io(&e, path))?;
    if !metadata.is_file() {
        return Err(IvError::new(ErrorCode::NotAFile, path));
    }
    let format = format_of(path).ok_or_else(|| IvError::new(ErrorCode::Unsupported, path))?;
    let path = fs::canonicalize(path).map_err(|e| IvError::from_io(&e, path))?;
    let path = strip_verbatim(path);
    Ok(ImageFile {
        name: path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
        path: path.display().to_string(),
        size: metadata.len(),
        format,
    })
}

/// `canonicalize` on Windows returns `\\?\C:\...`; the interface shows and uses `C:\...`.
fn strip_verbatim(path: PathBuf) -> PathBuf {
    let text = path.to_string_lossy();
    match text.strip_prefix(r"\\?\") {
        Some(rest) if !rest.starts_with("UNC\\") => PathBuf::from(rest),
        _ => path,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A copy of `testdata/` in a temporary folder: tests never touch anything else.
    fn testdata() -> tempfile::TempDir {
        let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata");
        let temp = tempfile::tempdir().expect("temp dir");
        for entry in fs::read_dir(&source).expect("testdata") {
            let entry = entry.expect("entry");
            if entry.file_type().expect("type").is_file() {
                fs::copy(entry.path(), temp.path().join(entry.file_name())).expect("copy");
            }
        }
        temp
    }

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn path_from_the_command_line() {
        let cwd = Path::new("/home/alex/photos");
        assert_eq!(path_from_args(&args(&["iv"]), cwd), None);
        assert_eq!(
            path_from_args(&args(&["iv", "a.jpg"]), cwd),
            Some(cwd.join("a.jpg"))
        );
        let absolute = std::env::temp_dir().join("b.png");
        assert_eq!(
            path_from_args(&args(&["iv", "--verbose", absolute.to_str().unwrap()]), cwd),
            Some(absolute)
        );
    }

    #[test]
    fn describes_every_image_of_testdata() {
        let temp = testdata();
        for name in [
            "red.png",
            "blue.bmp",
            "green.gif",
            "anim.webp",
            "sample.avif",
        ] {
            let file = describe(&temp.path().join(name)).expect(name);
            assert_eq!(file.name, name);
            assert_eq!(file.format, Format::Web);
            assert!(file.size > 0);
            assert!(Path::new(&file.path).is_absolute());
        }
    }

    #[test]
    fn webp_and_avif_samples_are_what_the_webview_expects() {
        let temp = testdata();
        let webp = fs::read(temp.path().join("anim.webp")).expect("webp");
        assert_eq!((&webp[0..4], &webp[8..12]), (&b"RIFF"[..], &b"WEBP"[..]));
        // Animated: an ANIM chunk and two ANMF frames (the WebView plays them).
        assert!(webp.windows(4).any(|w| w == b"ANIM"));
        assert_eq!(webp.windows(4).filter(|w| *w == b"ANMF").count(), 2);
        let avif = fs::read(temp.path().join("sample.avif")).expect("avif");
        assert_eq!(&avif[4..12], b"ftypavif");
        for name in ["broken.webp", "broken.avif"] {
            assert_eq!(
                describe(&temp.path().join(name)).expect(name).format,
                Format::Web
            );
        }
    }

    #[test]
    fn broken_image_is_still_opened_and_the_window_reports_it() {
        // Decoding errors are shown by the window (the image does not load), not refused here.
        let temp = testdata();
        let file = describe(&temp.path().join("broken.jpg")).expect("broken.jpg");
        assert_eq!(file.format, Format::Web);
    }

    #[test]
    fn errors_have_codes() {
        let temp = testdata();
        let code = |p: &Path| describe(p).expect_err("error").code;
        assert_eq!(code(&temp.path().join("missing.png")), ErrorCode::NotFound);
        assert_eq!(code(temp.path()), ErrorCode::NotAFile);
        assert_eq!(code(&temp.path().join("notes.txt")), ErrorCode::Unsupported);
    }

    #[test]
    fn describe_does_not_change_the_file() {
        let temp = testdata();
        let path = temp.path().join("red.png");
        let before = fs::read(&path).expect("read");
        let modified = fs::metadata(&path)
            .and_then(|m| m.modified())
            .expect("time");
        describe(&path).expect("describe");
        assert_eq!(fs::read(&path).expect("read"), before);
        assert_eq!(
            fs::metadata(&path)
                .and_then(|m| m.modified())
                .expect("time"),
            modified
        );
    }
}
