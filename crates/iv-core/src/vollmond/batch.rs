//! Batch processing (IV-19): a chain of steps — resize, convert, remove GPS or all metadata,
//! rename by the date taken — applied to copies. **The originals are never written to**: every
//! result goes to a new folder, a name that exists already gets " (2)", " (3)"… and is never
//! overwritten. Like every Vollmond tool it goes through the preview and the journal; undoing an
//! operation moves its copies to the Recycle Bin.

use std::fs;
use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};

use exif::{In, Reader, Tag};
use image::{DynamicImage, ImageDecoder, ImageReader};
use serde::{Deserialize, Serialize};

use super::dates::{date_from_name, Stamp};
use super::plan::{Change, Plan};
use crate::error::{ErrorCode, IvError};
use crate::exif::read_exif;
use crate::formats::{format_of, Format};

/// The tool name in plans and in the journal.
pub const TOOL: &str = "batch";

/// One step of a chain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "step", rename_all = "snake_case")]
pub enum Step {
    /// The long side at most this many pixels (never enlarged).
    Resize { long_side: u32 },
    /// The file at most this many bytes (JPEG quality, then size, go down until it fits).
    FitSize { max_bytes: u64 },
    /// The output format.
    Convert { to: Target },
    /// Remove the GPS position, keep the rest of the metadata.
    StripGps,
    /// Remove all metadata.
    StripAll,
    /// Name the copy by its date taken: `{YYYY}`, `{MM}`, `{DD}`, `{hh}`, `{mm}`, `{ss}`, `{name}`.
    RenameByDate { template: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Target {
    Jpeg,
    Png,
}

/// What the preview shows for one file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Planned {
    pub source: String,
    pub target: String,
    pub width: u32,
    pub height: u32,
}

/// The decoded, upright picture of a file (TIFF, HEIC and RAW previews through `decode`).
fn decode(path: &Path) -> Result<DynamicImage, IvError> {
    let bytes = match format_of(path) {
        Some(Format::Web) => fs::read(path).map_err(|e| IvError::from_io(&e, path))?,
        Some(Format::Decoded) => crate::decode::render(path)?.bytes,
        None => return Err(IvError::new(ErrorCode::Unsupported, path)),
    };
    let broken = || IvError::new(ErrorCode::Broken, path);
    let mut decoder = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|_| broken())?
        .into_decoder()
        .map_err(|_| broken())?;
    let orientation = decoder.orientation().map_err(|_| broken())?;
    let mut picture = DynamicImage::from_decoder(decoder).map_err(|_| broken())?;
    picture.apply_orientation(orientation);
    Ok(picture)
}

fn target_of(path: &Path, steps: &[Step]) -> Target {
    steps
        .iter()
        .rev()
        .find_map(|s| match s {
            Step::Convert { to } => Some(*to),
            _ => None,
        })
        .unwrap_or_else(|| {
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            if ext == "png" {
                Target::Png
            } else {
                Target::Jpeg
            }
        })
}

fn resized(picture: DynamicImage, long_side: u32) -> DynamicImage {
    if picture.width().max(picture.height()) <= long_side || long_side == 0 {
        picture
    } else {
        picture.resize(long_side, long_side, image::imageops::FilterType::Lanczos3)
    }
}

/// The date for renaming: EXIF, else the file name, else none (the name stays).
fn date_for_name(path: &Path) -> Option<Stamp> {
    read_exif(path)
        .and_then(|e| e.taken)
        .as_deref()
        .and_then(Stamp::parse)
        .or_else(|| {
            path.file_name()
                .and_then(|n| n.to_str())
                .and_then(date_from_name)
                .map(|d| d.stamp)
        })
}

/// The output file name (without the folder) for `source`.
pub fn output_name(source: &Path, steps: &[Step]) -> String {
    let stem = source
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("image")
        .to_owned();
    let ext = match target_of(source, steps) {
        Target::Jpeg => "jpg",
        Target::Png => "png",
    };
    let template = steps.iter().find_map(|s| match s {
        Step::RenameByDate { template } => Some(template.clone()),
        _ => None,
    });
    let name = match (template, date_for_name(source)) {
        (Some(t), Some(d)) => t
            .replace("{YYYY}", &format!("{:04}", d.year))
            .replace("{MM}", &format!("{:02}", d.month))
            .replace("{DD}", &format!("{:02}", d.day))
            .replace("{hh}", &format!("{:02}", d.hour))
            .replace("{mm}", &format!("{:02}", d.minute))
            .replace("{ss}", &format!("{:02}", d.second))
            .replace("{name}", &stem),
        _ => stem,
    };
    // No separators or reserved characters sneak in from a template.
    let clean: String = name
        .chars()
        .map(|c| if r#"\/:*?"<>|"#.contains(c) { '_' } else { c })
        .collect();
    format!("{clean}.{ext}")
}

/// A path in `folder` for `name` that does not exist yet: "name (2).jpg", "name (3).jpg"…
pub fn free_path(folder: &Path, name: &str, taken: &[PathBuf]) -> PathBuf {
    let (stem, ext) = name.rsplit_once('.').unwrap_or((name, ""));
    let mut n = 1;
    loop {
        let candidate = if n == 1 {
            folder.join(name)
        } else {
            folder.join(format!("{stem} ({n}).{ext}"))
        };
        if !candidate.exists() && !taken.contains(&candidate) {
            return candidate;
        }
        n += 1;
    }
}

/// The metadata of `source` for the copy: none, or everything but the GPS position (when asked),
/// the thumbnail and the maker's private notes, with Orientation 1 (the copy is upright).
fn copied_exif(source: &Path, strip_gps: bool) -> Option<Vec<u8>> {
    let file = fs::File::open(source).ok()?;
    let exif = Reader::new()
        .read_from_container(&mut std::io::BufReader::new(file))
        .ok()?;
    let fields: Vec<exif::Field> = exif
        .fields()
        .filter(|f| f.ifd_num == In::PRIMARY)
        .filter(|f| !(strip_gps && f.tag.context() == exif::Context::Gps))
        .filter(|f| !matches!(f.tag, Tag::MakerNote | Tag::Orientation))
        .cloned()
        .collect();
    let orientation = exif::Field {
        tag: Tag::Orientation,
        ifd_num: In::PRIMARY,
        value: exif::Value::Short(vec![1]),
    };
    let mut writer = exif::experimental::Writer::new();
    for f in &fields {
        writer.push_field(f);
    }
    writer.push_field(&orientation);
    let mut tiff = Cursor::new(Vec::new());
    writer.write(&mut tiff, false).ok()?;
    Some(tiff.into_inner())
}

fn with_exif(jpeg: Vec<u8>, tiff: &[u8]) -> Vec<u8> {
    let Ok(length) = u16::try_from(tiff.len() + 8) else {
        return jpeg;
    };
    let mut out = jpeg[..2].to_vec();
    out.extend_from_slice(&[0xFF, 0xE1]);
    out.extend_from_slice(&length.to_be_bytes());
    out.extend_from_slice(b"Exif\0\0");
    out.extend_from_slice(tiff);
    out.extend_from_slice(&jpeg[2..]);
    out
}

fn encode(picture: &DynamicImage, target: Target, quality: u8) -> Result<Vec<u8>, IvError> {
    let mut out = Cursor::new(Vec::new());
    let failed = |_| IvError::new(ErrorCode::Other, Path::new("encode"));
    match target {
        Target::Jpeg => image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, quality)
            .encode_image(&picture.to_rgb8())
            .map_err(failed)?,
        Target::Png => picture
            .write_to(&mut out, image::ImageFormat::Png)
            .map_err(failed)?,
    }
    Ok(out.into_inner())
}

/// The bytes of the processed copy of `source`.
pub fn process(source: &Path, steps: &[Step]) -> Result<Vec<u8>, IvError> {
    let mut picture = decode(source)?;
    for step in steps {
        if let Step::Resize { long_side } = step {
            picture = resized(picture, *long_side);
        }
    }
    let target = target_of(source, steps);
    let strip_all = steps.contains(&Step::StripAll);
    let strip_gps = steps.contains(&Step::StripGps);
    let metadata = if strip_all || target == Target::Png {
        None
    } else {
        copied_exif(source, strip_gps)
    };
    let finish = |bytes: Vec<u8>| match &metadata {
        Some(tiff) => with_exif(bytes, tiff),
        None => bytes,
    };
    let limit = steps.iter().find_map(|s| match s {
        Step::FitSize { max_bytes } => Some(*max_bytes),
        _ => None,
    });
    let Some(limit) = limit else {
        return Ok(finish(encode(&picture, target, 90)?));
    };
    // Quality first, then size, until it fits (a JPEG; PNG only by size).
    let mut current = picture;
    loop {
        let qualities: &[u8] = if target == Target::Jpeg {
            &[90, 82, 74, 66, 58, 50, 42]
        } else {
            &[100]
        };
        for &q in qualities {
            let bytes = finish(encode(&current, target, q)?);
            if bytes.len() as u64 <= limit {
                return Ok(bytes);
            }
        }
        let long = current.width().max(current.height());
        if long <= 64 {
            return Err(IvError::new(ErrorCode::TooLargeForLimit, source));
        }
        current = resized(current, long * 85 / 100);
    }
}

/// The preview of a batch: every source with its output path in `out_dir` (new names never
/// collide with each other or with files already there) and its output size.
pub fn plan(
    sources: &[PathBuf],
    steps: &[Step],
    out_dir: &Path,
) -> Result<(Plan, Vec<Planned>), IvError> {
    if sources.iter().any(|s| s.parent() == Some(out_dir)) {
        return Err(IvError::new(ErrorCode::SameFolder, out_dir));
    }
    let mut taken = Vec::new();
    let mut planned = Vec::new();
    let mut changes = Vec::new();
    for source in sources {
        let target = free_path(out_dir, &output_name(source, steps), &taken);
        taken.push(target.clone());
        let (w, h) = image::image_dimensions(source)
            .ok()
            .or_else(|| read_exif(source).and_then(|e| Some((e.width?, e.height?))))
            .unwrap_or((0, 0));
        // The copy is upright: a sideways orientation swaps the sides.
        let (w, h) = match read_exif(source).and_then(|e| e.orientation) {
            Some(5..=8) => (h, w),
            _ => (w, h),
        };
        let long = steps.iter().find_map(|s| match s {
            Step::Resize { long_side } => Some(*long_side),
            _ => None,
        });
        let (w, h) = match long {
            Some(l) if w.max(h) > l && l > 0 => {
                let k = f64::from(l) / f64::from(w.max(h));
                (
                    (f64::from(w) * k).round() as u32,
                    (f64::from(h) * k).round() as u32,
                )
            }
            _ => (w, h),
        };
        planned.push(Planned {
            source: source.display().to_string(),
            target: target.display().to_string(),
            width: w,
            height: h,
        });
        changes.push(Change {
            path: source.display().to_string(),
            field: "copy".into(),
            before: None,
            after: Some(target.display().to_string()),
            suspicious: false,
            // The chain goes into the journal with each copy: what was done, and how to redo it.
            note: serde_json::to_string(steps).ok(),
        });
    }
    Ok((
        Plan {
            tool: TOOL.into(),
            changes,
        },
        planned,
    ))
}

/// Apply one change: write the processed copy (never over an existing file); undo (`after` empty,
/// `before` the copy) moves the copy to the Recycle Bin. The original is only read.
/// `apply_change` with the steps from the change's note (as the journal keeps them).
pub fn apply_journalled(change: &Change) -> Result<(), IvError> {
    let steps: Vec<Step> = match &change.note {
        Some(json) => serde_json::from_str(json)
            .map_err(|_| IvError::new(ErrorCode::Other, Path::new(&change.path)))?,
        None => Vec::new(),
    };
    apply_change(change, &steps)
}

pub fn apply_change(change: &Change, steps: &[Step]) -> Result<(), IvError> {
    match (&change.before, &change.after) {
        (None, Some(target)) => {
            let target = Path::new(target);
            let bytes = process(Path::new(&change.path), steps)?;
            if let Some(dir) = target.parent() {
                fs::create_dir_all(dir).map_err(|e| IvError::from_io(&e, dir))?;
            }
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(target)
                .map_err(|e| IvError::from_io(&e, target))?;
            file.write_all(&bytes)
                .and_then(|()| file.sync_all())
                .map_err(|e| IvError::from_io(&e, target))
        }
        (Some(copy), None) => crate::recycle::move_to_trash(Path::new(copy)).map(|_| ()),
        _ => Err(IvError::new(
            ErrorCode::Unsupported,
            Path::new(&change.path),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exif::tests::{jpeg_with_exif, photo_fields};
    use crate::vollmond::plan::{apply, Journal};
    use std::sync::atomic::AtomicBool;

    fn testdata(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../testdata")
            .join(name)
    }

    /// A photo with full EXIF (GPS included) and real pixels: f-1.jpg's picture plus photo_fields.
    fn photo_with_gps(dir: &Path) -> PathBuf {
        let picture = fs::read(testdata("source/f-1.jpg")).expect("jpeg");
        let exif = jpeg_with_exif(&photo_fields());
        // jpeg_with_exif gives SOI + APP1 + EOI: put its APP1 into the real JPEG.
        let app1 = &exif[2..exif.len() - 2];
        let mut out = picture[..2].to_vec();
        out.extend_from_slice(app1);
        out.extend_from_slice(&picture[2..]);
        let path = dir.join("IMG_0001.jpg");
        fs::write(&path, out).expect("write");
        path
    }

    fn copy_of(dir: &Path, name: &str) -> PathBuf {
        let path = dir.join(name);
        fs::copy(testdata(name), &path).expect("copy");
        path
    }

    #[test]
    fn each_step_and_a_chain_leave_the_originals_alone() {
        let temp = tempfile::tempdir().expect("temp");
        let src = temp.path().join("src");
        fs::create_dir(&src).expect("mkdir");
        let photo = photo_with_gps(&src);
        let png = copy_of(&src, "red.png");
        let tif = copy_of(&src, "pages.tif");
        let originals: Vec<Vec<u8>> = [&photo, &png, &tif]
            .iter()
            .map(|p| fs::read(p).expect("read"))
            .collect();
        let out = temp.path().join("out");
        let steps = vec![
            Step::Resize { long_side: 100 },
            Step::Convert { to: Target::Jpeg },
            Step::StripGps,
            Step::RenameByDate {
                template: "{YYYY}-{MM}-{DD}_{hh}-{mm}-{ss}".into(),
            },
        ];
        let sources = vec![photo.clone(), png.clone(), tif.clone()];
        let (plan, planned) = plan(&sources, &steps, &out).expect("plan");
        assert_eq!(
            planned[0].target,
            out.join("2023-05-12_14-30-00.jpg").display().to_string()
        );
        // The photo says orientation 6: the copy is upright, 75×100.
        assert_eq!((planned[0].width, planned[0].height), (75, 100));
        assert!(planned[1].target.ends_with("red.jpg") && planned[2].target.ends_with("pages.jpg"));
        let journal = Journal::new(&temp.path().join("journal"));
        let report = apply(&plan, 0..3, &journal, None, &AtomicBool::new(false), |c| {
            apply_change(c, &steps)
        })
        .expect("apply");
        assert_eq!(report.error, None);
        // Originals byte for byte.
        for (p, o) in [&photo, &png, &tif].iter().zip(&originals) {
            assert_eq!(&fs::read(p).expect("read"), o);
        }
        // The copy: resized, JPEG, metadata kept without GPS, upright (orientation 1).
        let copy = out.join("2023-05-12_14-30-00.jpg");
        let info = read_exif(&copy).expect("exif kept");
        assert_eq!(info.camera.as_deref(), Some("Canon EOS R6"));
        assert_eq!(info.gps, None, "GPS removed");
        assert_eq!(info.orientation, Some(1));
        let bytes = fs::read(&copy).expect("copy");
        assert!(!bytes.windows(2).any(|w| w == b"N\0") || info.gps.is_none());
        let picture = image::load_from_memory(&bytes).expect("jpeg");
        assert_eq!((picture.width(), picture.height()), (75, 100));
    }

    #[test]
    fn strip_all_removes_every_piece_of_metadata() {
        let temp = tempfile::tempdir().expect("temp");
        let photo = photo_with_gps(temp.path());
        let bytes = process(&photo, &[Step::StripAll]).expect("process");
        assert!(Reader::new()
            .read_from_container(&mut Cursor::new(&bytes))
            .is_err());
        assert!(
            !bytes.windows(5).any(|w| w == b"Canon"),
            "no camera name left anywhere"
        );
    }

    #[test]
    fn fit_size_stays_under_the_limit() {
        let temp = tempfile::tempdir().expect("temp");
        let big = temp.path().join("big.png");
        image::RgbImage::from_fn(1200, 900, |x, y| {
            image::Rgb([
                (x * 7 % 256) as u8,
                (y * 3 % 256) as u8,
                ((x ^ y) % 256) as u8,
            ])
        })
        .save(&big)
        .expect("save");
        let limit = 60_000;
        let bytes = process(
            &big,
            &[
                Step::FitSize { max_bytes: limit },
                Step::Convert { to: Target::Jpeg },
            ],
        )
        .expect("fit");
        assert!(bytes.len() as u64 <= limit, "{} bytes", bytes.len());
        assert!(image::load_from_memory(&bytes).is_ok());
    }

    #[test]
    fn names_never_collide_and_the_source_folder_is_refused() {
        let temp = tempfile::tempdir().expect("temp");
        let out = temp.path().join("out");
        fs::create_dir(&out).expect("mkdir");
        fs::write(out.join("a.jpg"), b"already here").expect("write");
        let taken = vec![out.join("a (2).jpg")];
        assert_eq!(free_path(&out, "a.jpg", &taken), out.join("a (3).jpg"));
        let src = copy_of(temp.path(), "red.png");
        assert_eq!(
            plan(std::slice::from_ref(&src), &[], temp.path())
                .expect_err("same")
                .code,
            ErrorCode::SameFolder
        );
        // An existing file is never overwritten, even if it appears after the preview.
        let (p, _) = plan(&[src], &[], &out).expect("plan");
        let target = PathBuf::from(p.changes[0].after.clone().expect("after"));
        fs::write(&target, b"meanwhile").expect("write");
        assert!(apply_change(&p.changes[0], &[]).is_err());
        assert_eq!(fs::read(&target).expect("read"), b"meanwhile");
        assert_eq!(
            output_name(
                Path::new("x.jpg"),
                &[Step::RenameByDate {
                    template: "a/b:{name}".into()
                }]
            ),
            "x.jpg"
        );
    }
}
