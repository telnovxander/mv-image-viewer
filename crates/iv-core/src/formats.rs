//! Which files Spiegelmond shows, by extension (case does not matter).
//!
//! `Web` formats are shown by WebView2 itself through the asset protocol: hardware decoding,
//! animation and EXIF orientation for free (IV/PROJECT.md, "Двухуровневое декодирование").
//! Formats decoded by `iv-core` (IV-05): TIFF, HEIC/HEIF (libheif, loaded at run time), RAW previews.

use std::path::Path;

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Format {
    Web,
    /// Decoded by `iv-core` and served through the `ivimg` protocol (IV-05): TIFF, HEIF, RAW previews.
    Decoded,
}

/// Extensions WebView2 (Chromium) displays on its own.
pub const WEB_EXTENSIONS: &[&str] = &["jpg", "jpeg", "jfif", "png", "gif", "webp", "bmp", "avif"];

/// TIFF, decoded by `iv-core` (first page).
pub const TIFF_EXTENSIONS: &[&str] = &["tif", "tiff"];

/// HEIC / HEIF (HEVC inside: iPhone photos and others), decoded by libheif (`heif.rs`).
pub const HEIF_EXTENSIONS: &[&str] = &["heic", "heif", "hif"];

/// Camera RAW files: their largest embedded JPEG preview is shown (CR3 is ISO BMFF, the rest TIFF).
pub const RAW_EXTENSIONS: &[&str] = &[
    "dng", "cr2", "nef", "nrw", "arw", "srw", "orf", "rw2", "pef", "erf", "3fr", "cr3",
];

/// The format of a file by its extension, or `None` if Spiegelmond cannot show it.
pub fn format_of(path: &Path) -> Option<Format> {
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    if WEB_EXTENSIONS.contains(&extension.as_str()) {
        Some(Format::Web)
    } else if TIFF_EXTENSIONS.contains(&extension.as_str())
        || HEIF_EXTENSIONS.contains(&extension.as_str())
        || RAW_EXTENSIONS.contains(&extension.as_str())
    {
        Some(Format::Decoded)
    } else {
        None
    }
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
    fn tiff_and_raw_are_decoded_by_the_core() {
        for name in [
            "scan.TIF",
            "photo.dng",
            "photo.NEF",
            "photo.arw",
            "photo.cr2",
            "IMG_0001.CR3",
            "IMG_0001.HEIC",
            "photo.heif",
        ] {
            assert_eq!(format_of(Path::new(name)), Some(Format::Decoded), "{name}");
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
