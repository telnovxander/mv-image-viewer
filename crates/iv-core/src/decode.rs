//! Images WebView2 cannot show itself (IV-05), turned into something it can: TIFF → PNG of the
//! first page with its orientation applied; camera RAW (TIFF-based, and Canon CR3) → the largest
//! embedded JPEG preview with the
//! RAW's orientation written into it (the WebView turns it, nothing is re-encoded); HEIC/HEIF →
//! PNG of the primary image through libheif (`heif.rs`), upright. Reading only.

use std::fs;
use std::io::Cursor;
use std::path::Path;

use image::{ImageDecoder, ImageFormat};

use crate::error::{ErrorCode, IvError};
use crate::formats::{HEIF_EXTENSIONS, RAW_EXTENSIONS, TIFF_EXTENSIONS};
use crate::heif::{self, HeifFailure};

/// Bytes the window can show, with their MIME type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rendered {
    pub bytes: Vec<u8>,
    pub mime: &'static str,
}

fn broken(path: &Path) -> IvError {
    IvError::new(ErrorCode::Broken, path)
}

/// Render a TIFF, HEIF or RAW file for the window.
pub fn render(path: &Path) -> Result<Rendered, IvError> {
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    let data = fs::read(path).map_err(|e| IvError::from_io(&e, path))?;
    if TIFF_EXTENSIONS.contains(&extension.as_str()) {
        render_tiff(&data).ok_or_else(|| broken(path))
    } else if HEIF_EXTENSIONS.contains(&extension.as_str()) {
        match heif::render(&data) {
            Ok(bytes) => Ok(Rendered {
                bytes,
                mime: "image/png",
            }),
            Err(HeifFailure::NoLibrary) => Err(IvError::new(ErrorCode::NoCodec, path)),
            Err(HeifFailure::Broken) => Err(broken(path)),
        }
    } else if extension == "cr3" {
        cr3_preview(&data).ok_or_else(|| broken(path))
    } else if RAW_EXTENSIONS.contains(&extension.as_str()) {
        raw_preview(&data).ok_or_else(|| broken(path))
    } else {
        Err(IvError::new(ErrorCode::Unsupported, path))
    }
}

fn render_tiff(data: &[u8]) -> Option<Rendered> {
    let mut decoder = image::codecs::tiff::TiffDecoder::new(Cursor::new(data)).ok()?;
    let orientation = decoder.orientation().ok()?;
    let mut picture = image::DynamicImage::from_decoder(decoder).ok()?;
    picture.apply_orientation(orientation);
    let mut png = Cursor::new(Vec::new());
    picture.write_to(&mut png, ImageFormat::Png).ok()?;
    Some(Rendered {
        bytes: png.into_inner(),
        mime: "image/png",
    })
}

/// A little TIFF reader: enough to walk the IFDs of a RAW file and find its JPEG previews.
struct Tiff<'a> {
    data: &'a [u8],
    little: bool,
}

impl Tiff<'_> {
    fn u16(&self, at: usize) -> Option<u16> {
        let b = self.data.get(at..at + 2)?;
        Some(if self.little {
            u16::from_le_bytes([b[0], b[1]])
        } else {
            u16::from_be_bytes([b[0], b[1]])
        })
    }

    fn u32(&self, at: usize) -> Option<u32> {
        let b = self.data.get(at..at + 4)?;
        let b = [b[0], b[1], b[2], b[3]];
        Some(if self.little {
            u32::from_le_bytes(b)
        } else {
            u32::from_be_bytes(b)
        })
    }

    /// The entries of the IFD at `at`: (tag, type, count, value or offset position).
    fn entries(&self, at: usize) -> Vec<(u16, u16, u32, usize)> {
        let count = self.u16(at).map_or(0, usize::from).min(1000);
        (0..count)
            .filter_map(|i| {
                let entry = at + 2 + i * 12;
                Some((
                    self.u16(entry)?,
                    self.u16(entry + 2)?,
                    self.u32(entry + 4)?,
                    entry + 8,
                ))
            })
            .collect()
    }

    /// The first value of an entry as a number (SHORT or LONG, stored inline).
    fn value(&self, kind: u16, at: usize) -> Option<usize> {
        match kind {
            3 => self.u16(at).map(usize::from),
            4 => self.u32(at).and_then(|v| usize::try_from(v).ok()),
            _ => None,
        }
    }

    /// All values of a LONG array entry (sub-IFD offsets).
    fn longs(&self, count: u32, at: usize) -> Vec<usize> {
        let count = usize::try_from(count).unwrap_or(0).min(64);
        let start = if count <= 1 {
            Some(at)
        } else {
            self.u32(at).and_then(|o| usize::try_from(o).ok())
        };
        let Some(start) = start else {
            return Vec::new();
        };
        (0..count)
            .filter_map(|i| {
                self.u32(start + i * 4)
                    .and_then(|v| usize::try_from(v).ok())
            })
            .collect()
    }
}

fn raw_preview(data: &[u8]) -> Option<Rendered> {
    let little = match data.get(0..2)? {
        b"II" => true,
        b"MM" => false,
        _ => return None,
    };
    let tiff = Tiff { data, little };
    let mut queue = vec![usize::try_from(tiff.u32(4)?).ok()?];
    let mut seen = Vec::new();
    let mut best: Option<(usize, usize)> = None;
    let mut orientation = 1u16;
    while let Some(ifd) = queue.pop() {
        if ifd == 0 || ifd >= data.len() || seen.contains(&ifd) || seen.len() > 64 {
            continue;
        }
        seen.push(ifd);
        let entries = tiff.entries(ifd);
        let get = |tag: u16| {
            entries
                .iter()
                .find(|e| e.0 == tag)
                .and_then(|&(_, kind, _, at)| tiff.value(kind, at))
        };
        if seen.len() == 1 {
            orientation = get(0x0112)
                .and_then(|o| u16::try_from(o).ok())
                .filter(|o| (1..=8).contains(o))
                .unwrap_or(1);
        }
        // A JPEG preview: JPEGInterchangeFormat + length, or one strip of JPEG data.
        let candidate = match (get(0x0201), get(0x0202)) {
            (Some(offset), Some(length)) => Some((offset, length)),
            _ if matches!(get(0x0103), Some(6 | 7)) => get(0x0111).zip(get(0x0117)),
            _ => None,
        };
        if let Some((offset, length)) = candidate {
            let is_jpeg = data.get(offset..offset + 2) == Some(&[0xFF, 0xD8][..])
                && offset + length <= data.len();
            if is_jpeg && best.is_none_or(|(_, l)| length > l) {
                best = Some((offset, length));
            }
        }
        if let Some(&(_, _, count, at)) = entries.iter().find(|e| e.0 == 0x014A) {
            queue.extend(tiff.longs(count, at));
        }
        let next = ifd + 2 + entries.len() * 12;
        if let Some(next) = tiff.u32(next).and_then(|n| usize::try_from(n).ok()) {
            queue.push(next);
        }
    }
    let (offset, length) = best?;
    Some(Rendered {
        bytes: with_orientation(&data[offset..offset + length], orientation),
        mime: "image/jpeg",
    })
}

/// The payload of the first ISO BMFF box of kind `kind` anywhere in `data` (its size field and
/// end checked). CR3 nests its boxes in `moov` and Canon `uuid` boxes with private headers, so a
/// scan for the kind is simpler and as safe as walking them: only a checked range is returned.
fn find_box<'a>(data: &'a [u8], kind: &[u8; 4]) -> Option<&'a [u8]> {
    let mut from = 4;
    while from + 4 <= data.len() {
        let at = from + data.get(from..)?.windows(4).position(|w| w == kind)?;
        let start = at - 4;
        let size = data
            .get(start..at)
            .map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
            .and_then(|s| usize::try_from(s).ok())
            .unwrap_or(0);
        if size >= 8 && start + size <= data.len() {
            return data.get(at + 4..start + size);
        }
        from = at + 1;
    }
    None
}

/// Canon CR3: the JPEG preview of the `PRVW` box (1620×1080 in real files), turned like the
/// Orientation of IFD0 in the `CMT1` box (a little TIFF) says.
fn cr3_preview(data: &[u8]) -> Option<Rendered> {
    if data.get(4..12)? != b"ftypcrx " {
        return None;
    }
    let preview = find_box(data, b"PRVW")?;
    let start = preview.windows(3).position(|w| w == [0xFF, 0xD8, 0xFF])?;
    let jpeg = &preview[start..];
    let orientation = find_box(data, b"CMT1")
        .and_then(|tiff| {
            let little = match tiff.get(0..2)? {
                b"II" => true,
                b"MM" => false,
                _ => return None,
            };
            let reader = Tiff { data: tiff, little };
            let ifd = usize::try_from(reader.u32(4)?).ok()?;
            reader
                .entries(ifd)
                .into_iter()
                .find(|e| e.0 == 0x0112)
                .and_then(|(_, kind, _, at)| reader.value(kind, at))
        })
        .and_then(|o| u16::try_from(o).ok())
        .filter(|o| (1..=8).contains(o))
        .unwrap_or(1);
    Some(Rendered {
        bytes: with_orientation(jpeg, orientation),
        mime: "image/jpeg",
    })
}

/// The JPEG with an EXIF segment holding only `orientation`, replacing any EXIF it had: the
/// WebView then turns the preview like the RAW file says.
pub fn with_orientation(jpeg: &[u8], orientation: u16) -> Vec<u8> {
    let mut out = vec![0xFF, 0xD8];
    let mut tiff = b"II*\0\x08\0\0\0\x01\0".to_vec();
    tiff.extend_from_slice(&[0x12, 0x01, 3, 0, 1, 0, 0, 0]);
    tiff.extend_from_slice(&orientation.to_le_bytes());
    tiff.extend_from_slice(&[0, 0, 0, 0, 0, 0]);
    let length = u16::try_from(tiff.len() + 8).unwrap_or(u16::MAX);
    out.extend_from_slice(&[0xFF, 0xE1]);
    out.extend_from_slice(&length.to_be_bytes());
    out.extend_from_slice(b"Exif\0\0");
    out.extend_from_slice(&tiff);
    // Copy the segments after SOI, leaving out EXIF; the rest (scan data) as it is.
    let mut at = 2;
    while at + 4 <= jpeg.len() && jpeg[at] == 0xFF && jpeg[at + 1] != 0xDA {
        let length = usize::from(u16::from_be_bytes([jpeg[at + 2], jpeg[at + 3]]));
        let end = (at + 2 + length).min(jpeg.len());
        let is_exif = jpeg[at + 1] == 0xE1 && jpeg.get(at + 4..at + 10) == Some(b"Exif\0\0");
        if !is_exif {
            out.extend_from_slice(&jpeg[at..end]);
        }
        at = end;
    }
    out.extend_from_slice(jpeg.get(at..).unwrap_or_default());
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exif::read_exif;

    fn testdata(name: &str) -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../testdata")
            .join(name)
    }

    fn png_size(bytes: &[u8]) -> (u32, u32) {
        let picture = image::load_from_memory_with_format(bytes, ImageFormat::Png).expect("png");
        (picture.width(), picture.height())
    }

    #[test]
    fn tiff_first_page() {
        let rendered = render(&testdata("pages.tif")).expect("tiff");
        assert_eq!(rendered.mime, "image/png");
        // Page one is 64×40 (page two is 20×20).
        assert_eq!(png_size(&rendered.bytes), (64, 40));
    }

    #[test]
    fn raw_largest_preview_with_the_raw_orientation() {
        let rendered = render(&testdata("sample.dng")).expect("raw");
        assert_eq!(rendered.mime, "image/jpeg");
        let temp = tempfile::tempdir().expect("temp dir");
        let out = temp.path().join("preview.jpg");
        fs::write(&out, &rendered.bytes).expect("write");
        // The big preview (from orientation-1's pixels), turned like the RAW says (6).
        assert_eq!(read_exif(&out).and_then(|e| e.orientation), Some(6));
        assert!(
            rendered.bytes.len() > 2000,
            "the large preview, not the thumbnail"
        );
    }

    #[test]
    fn cr3_preview_with_its_orientation() {
        let rendered = render(&testdata("sample.cr3")).expect("cr3");
        assert_eq!(rendered.mime, "image/jpeg");
        let temp = tempfile::tempdir().expect("temp dir");
        let out = temp.path().join("preview.jpg");
        fs::write(&out, &rendered.bytes).expect("write");
        assert_eq!(read_exif(&out).and_then(|e| e.orientation), Some(8));
        // The whole preview, only the EXIF segment is new.
        let source = fs::read(testdata("source/f-1.jpg")).expect("source");
        assert_eq!(rendered.bytes, with_orientation(&source, 8));
    }

    #[test]
    fn broken_files_are_errors_not_panics() {
        for name in ["broken.tif", "broken.dng", "broken.cr3"] {
            assert_eq!(
                render(&testdata(name)).expect_err(name).code,
                ErrorCode::Broken,
                "{name}"
            );
        }
        assert_eq!(
            render(&testdata("missing.tif")).expect_err("missing").code,
            ErrorCode::NotFound
        );
        assert!(raw_preview(&[]).is_none());
        assert!(raw_preview(b"II*\0\xff\xff\xff\xff").is_none());
        assert!(cr3_preview(b"\0\0\0\x10ftypcrx \0\0\0\x01").is_none());
        // A PRVW box whose size runs past the end of the file is not trusted.
        assert!(find_box(
            b"\0\0\0\x10ftypcrx \xff\xff\xff\xffPRVW\xff\xd8\xff",
            b"PRVW"
        )
        .is_none());
    }

    /// HEIC needs libheif: on Windows CI the vendored DLLs (`vendor/heif/`) must load, elsewhere
    /// the system library is used when installed and the decoding tests are skipped otherwise.
    fn heif_or_skip() -> bool {
        let vendored = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../vendor/heif/heif.dll")
            .is_file();
        if cfg!(windows) && vendored {
            assert!(
                heif::available(),
                "vendor/heif/heif.dll is there but did not load"
            );
        }
        if !heif::available() {
            eprintln!("libheif not found: HEIC decoding tests skipped");
        }
        heif::available()
    }

    #[test]
    fn heic_primary_image_as_png() {
        if !heif_or_skip() {
            return;
        }
        let rendered = render(&testdata("sample.heic")).expect("heic");
        assert_eq!(rendered.mime, "image/png");
        // Made from source/f-1.jpg (160×120); the HEIC keeps its size.
        assert_eq!(png_size(&rendered.bytes), (160, 120));
    }

    #[test]
    fn broken_heic_is_an_error() {
        let code = render(&testdata("broken.heic")).expect_err("broken").code;
        if heif::available() {
            assert_eq!(code, ErrorCode::Broken);
        } else {
            assert_eq!(code, ErrorCode::NoCodec);
        }
    }

    #[test]
    fn orientation_segment_replaces_old_exif() {
        let source = fs::read(testdata("orientation-3.jpg")).expect("jpeg");
        let temp = tempfile::tempdir().expect("temp dir");
        let out = temp.path().join("x.jpg");
        fs::write(&out, with_orientation(&source, 8)).expect("write");
        assert_eq!(read_exif(&out).and_then(|e| e.orientation), Some(8));
    }
}
