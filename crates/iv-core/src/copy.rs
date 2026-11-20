//! Ctrl+C (IV-07): the image as the window shows it — EXIF orientation applied, then the view
//! rotation (R / Shift+R) — as RGBA pixels for the clipboard. Reading only.

use std::io::Cursor;
use std::path::Path;

use image::{DynamicImage, ImageDecoder, ImageReader};

use crate::error::{ErrorCode, IvError};
use crate::formats::{format_of, Format};

/// Pixels for the clipboard: width, height and RGBA bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pixels {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Decode bytes of any supported raster format, upright by their EXIF orientation.
fn upright(bytes: Vec<u8>) -> Option<DynamicImage> {
    let mut decoder = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .ok()?
        .into_decoder()
        .ok()?;
    let orientation = decoder.orientation().ok()?;
    let mut picture = DynamicImage::from_decoder(decoder).ok()?;
    picture.apply_orientation(orientation);
    Some(picture)
}

/// The image at `path` as shown with the view rotation `degrees` (0, 90, 180, 270, clockwise).
pub fn pixels(path: &Path, degrees: u16) -> Result<Pixels, IvError> {
    let bytes = match format_of(path) {
        Some(Format::Web) => std::fs::read(path).map_err(|e| IvError::from_io(&e, path))?,
        Some(Format::Decoded) => crate::decode::render(path)?.bytes,
        None => return Err(IvError::new(ErrorCode::Unsupported, path)),
    };
    // AVIF cannot be decoded here (WebView2 shows it itself): "cannot copy" rather than "broken".
    let picture = upright(bytes).ok_or_else(|| IvError::new(ErrorCode::CopyFailed, path))?;
    let picture = match degrees % 360 {
        90 => picture.rotate90(),
        180 => picture.rotate180(),
        270 => picture.rotate270(),
        _ => picture,
    };
    let rgba = picture.into_rgba8();
    Ok(Pixels {
        width: rgba.width(),
        height: rgba.height(),
        rgba: rgba.into_raw(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn testdata(name: &str) -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../testdata")
            .join(name)
    }

    fn rgb(p: &Pixels, x: u32, y: u32) -> [u8; 3] {
        let at = ((y * p.width + x) * 4) as usize;
        [p.rgba[at], p.rgba[at + 1], p.rgba[at + 2]]
    }

    #[test]
    fn every_orientation_comes_out_the_same() {
        // orientation-<n>.jpg all show the same upright F when the orientation is applied.
        let reference = pixels(&testdata("orientation-1.jpg"), 0).expect("1");
        for n in 2..=8 {
            let other = pixels(&testdata(&format!("orientation-{n}.jpg")), 0).expect("jpeg");
            assert_eq!(
                (other.width, other.height),
                (reference.width, reference.height),
                "{n}"
            );
            let differs = reference
                .rgba
                .iter()
                .zip(&other.rgba)
                .filter(|(a, b)| a.abs_diff(**b) > 48)
                .count();
            assert!(
                differs < reference.rgba.len() / 50,
                "orientation {n}: {differs}"
            );
        }
    }

    #[test]
    fn view_rotation_turns_the_copy() {
        let upright = pixels(&testdata("red.png"), 0).expect("png");
        assert_eq!((upright.width, upright.height), (64, 48));
        let turned = pixels(&testdata("red.png"), 90).expect("png");
        assert_eq!((turned.width, turned.height), (48, 64));
        assert_eq!(rgb(&turned, 0, 0), [200, 40, 40]);
        assert_eq!(pixels(&testdata("red.png"), 180).expect("png").width, 64);
    }

    #[test]
    fn decoded_formats_and_errors() {
        let tiff = pixels(&testdata("pages.tif"), 0).expect("tiff");
        assert_eq!((tiff.width, tiff.height), (64, 40));
        let raw = pixels(&testdata("sample.dng"), 0).expect("raw");
        // The RAW says orientation 6: the 160×120 preview comes out 120×160.
        assert_eq!((raw.width, raw.height), (120, 160));
        assert_eq!(
            pixels(&testdata("broken.jpg"), 0).expect_err("broken").code,
            ErrorCode::CopyFailed
        );
        assert_eq!(
            pixels(&testdata("notes.txt"), 0).expect_err("txt").code,
            ErrorCode::Unsupported
        );
        assert_eq!(
            pixels(&testdata("missing.png"), 0)
                .expect_err("missing")
                .code,
            ErrorCode::NotFound
        );
    }
}
