//! Thumbnails of the film strip (IV-08), shared with the Vollmond index (IV-13): the same folder
//! and format, so a thumbnail is never built twice.
//!
//! A thumbnail is a JPEG of at most [`THUMB_SIZE`] pixels on the long side. The embedded EXIF
//! thumbnail is used when the photo has one (no decoding of a 50 MP photo), with the photo's
//! orientation written into it; otherwise the image is decoded and scaled down, upright.
//!
//! Disk cache, format 1: `<cache>/v1/<key>.jpg`, where the key is FNV-1a 64 of the path, the file
//! size and its modification time — a changed file gets a new key, the old entry ages out. The
//! folder is kept under a size limit by deleting the least recently written entries. Reading only:
//! the photos themselves are never touched.

use std::fs;
use std::io::{BufReader, Cursor};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use image::{DynamicImage, ImageDecoder, ImageReader};

use crate::decode::with_orientation;
use crate::error::{ErrorCode, IvError};
use crate::formats::{format_of, Format};

/// The long side of a thumbnail, in pixels (the strip shows them at 64–96 px; 2× for HiDPI).
pub const THUMB_SIZE: u32 = 192;

/// The default limit of the disk cache.
pub const CACHE_LIMIT: u64 = 256 * 1024 * 1024;

/// Version of the cache format: a new format goes to a new folder.
const FORMAT: &str = "v1";

/// FNV-1a, 64 bits: a stable key across versions of Rust (unlike `DefaultHasher`).
fn fnv1a(data: &[u8], mut hash: u64) -> u64 {
    for &byte in data {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01B3);
    }
    hash
}

/// The cache key of a file in its current state: path, size and modification time.
pub fn cache_key(path: &Path) -> Result<String, IvError> {
    let metadata = fs::metadata(path).map_err(|e| IvError::from_io(&e, path))?;
    let modified = metadata
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_nanos());
    let mut hash = fnv1a(path.to_string_lossy().as_bytes(), 0xCBF2_9CE4_8422_2325);
    hash = fnv1a(&metadata.len().to_le_bytes(), hash);
    hash = fnv1a(&modified.to_le_bytes(), hash);
    Ok(format!("{hash:016x}"))
}

/// The embedded EXIF thumbnail of a JPEG, turned like the photo (orientation written into it).
fn exif_thumbnail(path: &Path) -> Option<Vec<u8>> {
    let file = fs::File::open(path).ok()?;
    let exif = exif::Reader::new()
        .read_from_container(&mut BufReader::new(file))
        .ok()?;
    let number = |tag, ifd| {
        exif.get_field(tag, ifd)
            .and_then(|f| f.value.get_uint(0))
            .and_then(|v| usize::try_from(v).ok())
    };
    let offset = number(exif::Tag::JPEGInterchangeFormat, exif::In::THUMBNAIL)?;
    let length = number(exif::Tag::JPEGInterchangeFormatLength, exif::In::THUMBNAIL)?;
    let jpeg = exif.buf().get(offset..offset.checked_add(length)?)?;
    if !jpeg.starts_with(&[0xFF, 0xD8]) {
        return None;
    }
    let orientation = number(exif::Tag::Orientation, exif::In::PRIMARY)
        .and_then(|o| u16::try_from(o).ok())
        .filter(|o| (1..=8).contains(o))
        .unwrap_or(1);
    Some(with_orientation(jpeg, orientation))
}

/// Decode, scale down and turn upright; then JPEG.
fn scaled(path: &Path) -> Result<Vec<u8>, IvError> {
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
    let picture = DynamicImage::from_decoder(decoder).map_err(|_| broken())?;
    // Only ever smaller: `thumbnail` would also enlarge a small picture.
    let mut small = if picture.width() > THUMB_SIZE || picture.height() > THUMB_SIZE {
        picture.thumbnail(THUMB_SIZE, THUMB_SIZE)
    } else {
        picture
    };
    small.apply_orientation(orientation);
    let mut jpeg = Cursor::new(Vec::new());
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 82)
        .encode_image(&small.into_rgb8())
        .map_err(|_| broken())?;
    Ok(jpeg.into_inner())
}

/// A thumbnail of `path`, built now (no cache): the EXIF one when there is one.
pub fn make(path: &Path) -> Result<Vec<u8>, IvError> {
    if let Some(jpeg) = exif_thumbnail(path) {
        return Ok(jpeg);
    }
    scaled(path)
}

/// The disk cache of thumbnails.
#[derive(Debug, Clone)]
pub struct ThumbCache {
    dir: PathBuf,
    limit: u64,
}

impl ThumbCache {
    /// A cache in `root` (the folder `v1` is made inside), at most `limit` bytes.
    pub fn new(root: &Path, limit: u64) -> Self {
        Self {
            dir: root.join(FORMAT),
            limit,
        }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The thumbnail of `path` from the cache, or built and stored. A cache that cannot be
    /// written (read-only, full disk) still gives the thumbnail.
    pub fn get(&self, path: &Path) -> Result<Vec<u8>, IvError> {
        let entry = self.dir.join(format!("{}.jpg", cache_key(path)?));
        if let Ok(bytes) = fs::read(&entry) {
            return Ok(bytes);
        }
        let jpeg = make(path)?;
        if fs::create_dir_all(&self.dir).is_ok() {
            // Written next to the entry, then renamed: a reader never sees half a JPEG.
            let part = entry.with_extension(format!("{}.part", std::process::id()));
            if fs::write(&part, &jpeg).is_ok() && fs::rename(&part, &entry).is_err() {
                let _ = fs::remove_file(&part);
            }
        }
        Ok(jpeg)
    }

    /// Make sure the thumbnail of `path` is in the cache: `Ok(true)` when it was built now,
    /// `Ok(false)` when it was there already.
    pub fn ensure(&self, path: &Path) -> Result<bool, IvError> {
        let entry = self.dir.join(format!("{}.jpg", cache_key(path)?));
        if entry.is_file() {
            return Ok(false);
        }
        self.get(path).map(|_| true)
    }

    /// Delete the oldest entries until the cache is under its limit; the bytes left.
    pub fn trim(&self) -> u64 {
        let Ok(items) = fs::read_dir(&self.dir) else {
            return 0;
        };
        let mut files: Vec<(std::time::SystemTime, u64, PathBuf)> = items
            .filter_map(Result::ok)
            .filter_map(|item| {
                let metadata = item.metadata().ok()?;
                metadata.is_file().then(|| {
                    (
                        metadata.modified().unwrap_or(UNIX_EPOCH),
                        metadata.len(),
                        item.path(),
                    )
                })
            })
            .collect();
        let mut total: u64 = files.iter().map(|f| f.1).sum();
        files.sort();
        for (_, size, path) in files {
            if total <= self.limit {
                break;
            }
            if fs::remove_file(&path).is_ok() {
                total = total.saturating_sub(size);
            }
        }
        total
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exif::read_exif;

    fn testdata(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../testdata")
            .join(name)
    }

    /// A copy of a test file in a temporary folder.
    fn copy(temp: &Path, name: &str) -> PathBuf {
        let target = temp.join(name);
        fs::copy(testdata(name), &target).expect("copy");
        target
    }

    fn size_of(jpeg: &[u8]) -> (u32, u32) {
        let picture = image::load_from_memory(jpeg).expect("jpeg");
        (picture.width(), picture.height())
    }

    #[test]
    fn exif_thumbnail_first_with_the_photo_orientation() {
        let temp = tempfile::tempdir().expect("temp");
        let photo = copy(temp.path(), "thumb-6.jpg");
        let thumb = make(&photo).expect("thumb");
        // The 40×30 EXIF thumbnail, not a scaled copy of the photo.
        assert_eq!(size_of(&thumb), (40, 30));
        let out = temp.path().join("t.jpg");
        fs::write(&out, &thumb).expect("write");
        assert_eq!(read_exif(&out).and_then(|e| e.orientation), Some(6));
    }

    #[test]
    fn scaled_and_upright_without_exif_thumbnail() {
        let temp = tempfile::tempdir().expect("temp");
        // 120×160 stored, orientation 6: upright it is 160×120.
        let thumb = make(&copy(temp.path(), "orientation-6.jpg")).expect("jpeg");
        assert_eq!(size_of(&thumb), (160, 120));
        assert_eq!(
            size_of(&make(&copy(temp.path(), "red.png")).expect("png")),
            (64, 48)
        );
        assert_eq!(
            size_of(&make(&copy(temp.path(), "pages.tif")).expect("tiff")),
            (64, 40)
        );
        assert_eq!(
            make(&copy(temp.path(), "broken.jpg"))
                .expect_err("broken")
                .code,
            ErrorCode::Broken
        );
    }

    #[test]
    fn large_pictures_shrink_to_the_thumbnail_size() {
        let temp = tempfile::tempdir().expect("temp");
        let big = temp.path().join("big.png");
        image::RgbImage::from_pixel(800, 600, image::Rgb([10, 120, 200]))
            .save(&big)
            .expect("save");
        assert_eq!(size_of(&make(&big).expect("big")), (THUMB_SIZE, 144));
    }

    #[test]
    fn cache_hits_and_a_changed_file_gets_a_new_thumbnail() {
        let temp = tempfile::tempdir().expect("temp");
        let photo = copy(temp.path(), "red.png");
        let cache = ThumbCache::new(&temp.path().join("cache"), CACHE_LIMIT);
        let first = cache.get(&photo).expect("first");
        let entries = || fs::read_dir(cache.dir()).expect("dir").count();
        assert_eq!(entries(), 1);
        assert_eq!(cache.get(&photo).expect("hit"), first);
        assert_eq!(entries(), 1);
        // The file changes (another picture, other size): new key, new thumbnail.
        fs::copy(testdata("blue.bmp"), &photo).expect("replace");
        let second = cache.get(&photo).expect("changed");
        assert_ne!(second, first);
        assert_eq!(size_of(&second), (48, 64));
        assert_eq!(entries(), 2);
        // The photos themselves are untouched.
        assert_eq!(
            fs::read(&photo).expect("read"),
            fs::read(testdata("blue.bmp")).expect("bmp")
        );
    }

    #[test]
    fn trim_keeps_the_cache_under_its_limit() {
        let temp = tempfile::tempdir().expect("temp");
        let cache = ThumbCache::new(&temp.path().join("cache"), 2500);
        fs::create_dir_all(cache.dir()).expect("mkdir");
        for i in 0..5 {
            fs::write(cache.dir().join(format!("{i}.jpg")), vec![0u8; 1000]).expect("write");
        }
        assert_eq!(cache.trim(), 2000);
        assert_eq!(fs::read_dir(cache.dir()).expect("dir").count(), 2);
    }

    #[test]
    fn keys_are_stable() {
        assert_eq!(fnv1a(b"", 0xCBF2_9CE4_8422_2325), 0xCBF2_9CE4_8422_2325);
        assert_eq!(fnv1a(b"a", 0xCBF2_9CE4_8422_2325), 0xAF63_DC4C_8601_EC8C);
    }
}
