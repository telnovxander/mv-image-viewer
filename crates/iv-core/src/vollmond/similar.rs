//! Similar photos (IV-18): bursts, the same photo in other sizes or qualities, edited copies.
//!
//! Each picture gets a 64-bit difference hash (dHash) of its thumbnail (the shared cache of
//! IV-08): a 9×8 grey version, one bit per neighbour comparison. Two pictures are similar when
//! their hashes differ in at most `threshold` bits. The hashes are kept in the index (IV-13).
//!
//! Nothing is ever marked for deletion automatically (point 18): the "best frame" — sharpness,
//! resolution, exposure — is only a hint. Deleting the files the user chose goes through the same
//! safe path as exact duplicates (`duplicates::plan`: never a whole group, never a protected
//! folder; the Recycle Bin only; undo).

use std::sync::atomic::{AtomicBool, Ordering};

use image::DynamicImage;
use serde::Serialize;

use super::duplicates::Group;
use super::index::{Index, Indexed};

/// The default threshold: up to 10 differing bits of 64.
pub const DEFAULT_THRESHOLD: u32 = 10;

/// dHash of a picture: 64 bits, robust to size and compression changes.
pub fn dhash(picture: &DynamicImage) -> u64 {
    let small = picture
        .grayscale()
        .resize_exact(9, 8, image::imageops::FilterType::Triangle)
        .to_luma8();
    let mut hash = 0u64;
    for y in 0..8 {
        for x in 0..8 {
            hash <<= 1;
            if small.get_pixel(x, y)[0] < small.get_pixel(x + 1, y)[0] {
                hash |= 1;
            }
        }
    }
    hash
}

/// How good a frame looks, for the hint: sharpness (variance of the Laplacian of the thumbnail),
/// resolution, and how far the brightness is from too dark / too bright.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Quality {
    pub sharpness: f64,
    pub pixels: u64,
    /// 0 (all black or all white) … 1 (well exposed).
    pub exposure: f64,
}

pub fn quality(picture: &DynamicImage, pixels: u64) -> Quality {
    let grey = picture.to_luma8();
    let (w, h) = grey.dimensions();
    let mut values = Vec::new();
    for y in 1..h.saturating_sub(1) {
        for x in 1..w.saturating_sub(1) {
            let c = f64::from(grey.get_pixel(x, y)[0]);
            let around = f64::from(grey.get_pixel(x - 1, y)[0])
                + f64::from(grey.get_pixel(x + 1, y)[0])
                + f64::from(grey.get_pixel(x, y - 1)[0])
                + f64::from(grey.get_pixel(x, y + 1)[0]);
            values.push(around - 4.0 * c);
        }
    }
    let n = values.len().max(1) as f64;
    let mean = values.iter().sum::<f64>() / n;
    let sharpness = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n;
    let count = f64::from(w * h).max(1.0);
    let brightness = grey.pixels().map(|p| f64::from(p[0])).sum::<f64>() / count / 255.0;
    let clipped = grey.pixels().filter(|p| p[0] < 5 || p[0] > 250).count() as f64 / count;
    let exposure = ((1.0 - (brightness - 0.5).abs() * 2.0) * (1.0 - clipped)).clamp(0.0, 1.0);
    Quality {
        sharpness,
        pixels,
        exposure,
    }
}

/// The index of the best-looking frame of a group (only a hint).
pub fn best(qualities: &[Quality]) -> usize {
    let max_sharp = qualities
        .iter()
        .map(|q| q.sharpness)
        .fold(0.0, f64::max)
        .max(1e-9);
    let max_pixels = qualities.iter().map(|q| q.pixels).max().unwrap_or(1).max(1) as f64;
    let score = |q: &Quality| {
        0.5 * q.sharpness / max_sharp + 0.3 * q.pixels as f64 / max_pixels + 0.2 * q.exposure
    };
    (0..qualities.len())
        .max_by(|&a, &b| score(&qualities[a]).total_cmp(&score(&qualities[b])))
        .unwrap_or(0)
}

/// A group of similar pictures, with the hint.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SimilarGroup {
    pub files: Vec<Indexed>,
    pub qualities: Vec<Quality>,
    /// The suggested best frame: a hint, never a choice made for the user.
    pub best: usize,
}

impl SimilarGroup {
    /// As a duplicate group for the shared deletion plan (`keep` = the hint; nothing chosen).
    pub fn as_group(&self) -> Group {
        Group {
            files: self.files.clone(),
            keep: self.best,
        }
    }
}

/// Progress: pictures fingerprinted of `total`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Progress {
    pub done: usize,
    pub total: usize,
}

/// The thumbnail of `image` decoded (from the shared cache when there is one).
fn thumbnail(image: &Indexed, cache: Option<&crate::thumbs::ThumbCache>) -> Option<DynamicImage> {
    let path = std::path::Path::new(&image.path);
    let jpeg = match cache {
        Some(cache) => cache.get(path).ok()?,
        None => crate::thumbs::make(path).ok()?,
    };
    image::load_from_memory(&jpeg).ok()
}

/// Find groups of similar pictures among `images` (videos and undecodable files are skipped).
pub fn find(
    index: &Index,
    images: &[Indexed],
    threshold: u32,
    cache: Option<&crate::thumbs::ThumbCache>,
    cancel: &AtomicBool,
    mut on_progress: impl FnMut(Progress),
) -> Vec<SimilarGroup> {
    let total = images.len();
    let mut prints: Vec<(usize, u64)> = Vec::new();
    for (i, image) in images.iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            return Vec::new();
        }
        if super::video::is_video(std::path::Path::new(&image.path)) {
            continue;
        }
        let print = match index.picture_print(image) {
            Some(p) => Some(p),
            None => thumbnail(image, cache).map(|t| {
                let p = dhash(&t);
                let _ = index.store_picture_print(image, p);
                p
            }),
        };
        if let Some(p) = print {
            prints.push((i, p));
        }
        if (i + 1) % 100 == 0 || i + 1 == total {
            on_progress(Progress { done: i + 1, total });
        }
    }
    // Union of every pair within the threshold.
    let mut parent: Vec<usize> = (0..prints.len()).collect();
    fn root(parent: &mut [usize], mut x: usize) -> usize {
        while parent[x] != x {
            parent[x] = parent[parent[x]];
            x = parent[x];
        }
        x
    }
    for a in 0..prints.len() {
        if a % 1000 == 0 && cancel.load(Ordering::Relaxed) {
            return Vec::new();
        }
        for b in a + 1..prints.len() {
            if (prints[a].1 ^ prints[b].1).count_ones() <= threshold {
                let (ra, rb) = (root(&mut parent, a), root(&mut parent, b));
                if ra != rb {
                    parent[rb] = ra;
                }
            }
        }
    }
    let mut members: std::collections::BTreeMap<usize, Vec<usize>> = Default::default();
    for (k, &(image, _)) in prints.iter().enumerate() {
        let r = root(&mut parent, k);
        members.entry(r).or_default().push(image);
    }
    members
        .into_values()
        .filter(|m| m.len() > 1)
        .map(|m| {
            let files: Vec<Indexed> = m.iter().map(|&i| images[i].clone()).collect();
            let qualities: Vec<Quality> = files
                .iter()
                .map(|f| {
                    let pixels = u64::from(f.width.unwrap_or(0)) * u64::from(f.height.unwrap_or(0));
                    thumbnail(f, cache).map_or(
                        Quality {
                            sharpness: 0.0,
                            pixels,
                            exposure: 0.0,
                        },
                        |t| {
                            let pixels = if pixels == 0 {
                                u64::from(t.width()) * u64::from(t.height())
                            } else {
                                pixels
                            };
                            quality(&t, pixels)
                        },
                    )
                })
                .collect();
            let best = best(&qualities);
            SimilarGroup {
                files,
                qualities,
                best,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Cursor;
    use std::path::{Path, PathBuf};

    fn testdata(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../testdata")
            .join(name)
    }

    fn jpeg(picture: &DynamicImage, quality: u8) -> Vec<u8> {
        let mut out = Cursor::new(Vec::new());
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, quality)
            .encode_image(&picture.to_rgb8())
            .expect("encode");
        out.into_inner()
    }

    /// A scene, its half-size copy, a strongly recompressed copy; and other scenes.
    fn archive() -> tempfile::TempDir {
        let temp = tempfile::tempdir().expect("temp");
        let root = temp.path();
        let scene = image::open(testdata("source/f-1.jpg")).expect("scene");
        fs::write(root.join("scene.jpg"), jpeg(&scene, 92)).expect("write");
        fs::write(
            root.join("scene-small.jpg"),
            jpeg(
                &scene.resize(80, 60, image::imageops::FilterType::Lanczos3),
                85,
            ),
        )
        .expect("write");
        fs::write(root.join("scene-q20.jpg"), jpeg(&scene, 20)).expect("write");
        // Other scenes: the F upside down, and a gradient.
        fs::write(root.join("other.jpg"), jpeg(&scene.rotate180(), 92)).expect("write");
        let gradient = DynamicImage::ImageRgb8(image::RgbImage::from_fn(160, 120, |x, y| {
            image::Rgb([x as u8, y as u8 * 2, 90])
        }));
        fs::write(root.join("gradient.jpg"), jpeg(&gradient, 92)).expect("write");
        temp
    }

    fn groups(root: &Path, threshold: u32) -> Vec<Vec<String>> {
        let mut index = Index::in_memory().expect("db");
        index
            .index(root, false, false, &AtomicBool::new(false), |_| {})
            .expect("index");
        let images = index.images(root, false).expect("images");
        let mut found: Vec<Vec<String>> = find(
            &index,
            &images,
            threshold,
            None,
            &AtomicBool::new(false),
            |_| {},
        )
        .into_iter()
        .map(|g| {
            let mut names: Vec<String> = g
                .files
                .iter()
                .map(|f| f.path.rsplit(['/', '\\']).next().unwrap_or("").to_owned())
                .collect();
            names.sort();
            names
        })
        .collect();
        found.sort();
        found
    }

    #[test]
    fn resized_and_recompressed_copies_are_similar_other_scenes_not() {
        let temp = archive();
        assert_eq!(
            groups(temp.path(), DEFAULT_THRESHOLD),
            [["scene-q20.jpg", "scene-small.jpg", "scene.jpg"]]
        );
    }

    #[test]
    fn the_threshold_matters() {
        let temp = archive();
        assert!(groups(temp.path(), 0).len() <= 1);
        assert!(
            groups(temp.path(), 64).iter().any(|g| g.len() == 5),
            "everything is similar at 64"
        );
    }

    #[test]
    fn the_hint_prefers_sharp_big_well_exposed_frames() {
        let scene = image::open(testdata("source/f-1.jpg")).expect("scene");
        let blurred = scene.blur(3.0);
        let dark = scene.brighten(-120);
        let q = [
            quality(&blurred, 100),
            quality(&scene, 100),
            quality(&dark, 100),
        ];
        assert_eq!(best(&q), 1);
        assert!(q[1].exposure > q[2].exposure);
    }

    #[test]
    fn prints_are_kept_in_the_index() {
        let temp = archive();
        let mut index = Index::in_memory().expect("db");
        index
            .index(temp.path(), false, false, &AtomicBool::new(false), |_| {})
            .expect("index");
        let images = index.images(temp.path(), false).expect("images");
        assert!(index.picture_print(&images[0]).is_none());
        find(
            &index,
            &images,
            DEFAULT_THRESHOLD,
            None,
            &AtomicBool::new(false),
            |_| {},
        );
        assert!(images.iter().all(|i| index.picture_print(i).is_some()));
    }
}
