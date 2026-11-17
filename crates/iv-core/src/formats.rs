//! Which files Spiegelmond shows, by extension (case does not matter).
//!
//! `Web` formats are shown by WebView2 itself through the asset protocol: hardware decoding,
//! animation and EXIF orientation for free (IV/PROJECT.md, "Двухуровневое декодирование").
//! Formats decoded by `iv-core` (TIFF, HEIC, RAW previews) come with IV-05.

use std::path::Path;

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Format {
    Web,
}

/// Extensions WebView2 (Chromium) displays on its own.
pub const WEB_EXTENSIONS: &[&str] = &["jpg", "jpeg", "jfif", "png", "gif", "webp", "bmp", "avif"];

/// The format of a file by its extension, or `None` if Spiegelmond cannot show it.
pub fn format_of(path: &Path) -> Option<Format> {
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    WEB_EXTENSIONS
        .contains(&extension.as_str())
        .then_some(Format::Web)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn web_formats_in_any_case() {
        for name in [
            "a.jpg", "b.JPEG", "c.Png", "d.gif", "e.webp", "f.BMP", "g.avif", "h.jfif",
        ] {
            assert_eq!(format_of(Path::new(name)), Some(Format::Web), "{name}");
        }
    }

    #[test]
    fn other_files_are_not_images() {
        for name in [
            "notes.txt",
            "photo",
            "archive.zip",
            ".jpg",
            "movie.mp4",
            "photo.jpg.part",
        ] {
            assert_eq!(format_of(Path::new(name)), None, "{name}");
        }
    }
}
