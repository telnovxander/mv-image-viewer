//! The index of a photo archive (IV-13): which images a folder holds, their EXIF and content
//! hashes, in SQLite next to the thumbnail cache. Reading the photos only — the index never
//! changes a file.
//!
//! Indexing runs in two passes, both in the background with progress and cancel:
//! 1. **files**: walk the folder (with or without subfolders); an image whose size and modification
//!    time match the index is skipped (so opening the same folder again is instant), a new or
//!    changed one gets its EXIF read again, a vanished one leaves the index;
//! 2. **hashes**: BLAKE3 of the content of every file without a hash yet (the slow part: it reads
//!    every byte; a cancelled pass goes on where it stopped next time);
//! 3. **thumbnails**: into the cache the film strip uses (IV-08), so they are built once.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::UNIX_EPOCH;

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use crate::error::{ErrorCode, IvError};
use crate::exif::read_exif;
use crate::formats::format_of;

/// Version of the database layout; another number starts a fresh index.
const SCHEMA: i64 = 1;

/// Progress is reported at most every this many files.
const REPORT_EVERY: u64 = 250;

/// One image of the index.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Indexed {
    pub path: String,
    pub size: u64,
    pub modified: i64,
    /// EXIF DateTimeOriginal as `YYYY-MM-DDTHH:MM:SS`.
    pub taken: Option<String>,
    pub camera: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    /// BLAKE3 of the content, hex; `None` until the hash pass reaches the file.
    pub hash: Option<String>,
}

/// Where indexing is, for the progress bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "phase", rename_all = "snake_case")]
pub enum Progress {
    /// Files seen so far (their number is not known in advance).
    Files { seen: u64 },
    /// Hashes done of `total`.
    Hashes { done: u64, total: u64 },
    /// Thumbnails done of `total`.
    Thumbnails { done: u64, total: u64 },
}

/// What indexing did.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct IndexSummary {
    pub images: u64,
    pub added: u64,
    pub changed: u64,
    pub unchanged: u64,
    pub removed: u64,
    pub hashed: u64,
    /// Thumbnails built (the ones already in the cache are not counted).
    pub thumbnails: u64,
    /// Folders that could not be read (no access, vanished).
    pub skipped_folders: u64,
    pub cancelled: bool,
}

/// The index database of one installation (all folders in one file).
pub struct Index {
    db: Connection,
}

fn sql_error(path: &Path) -> impl Fn(rusqlite::Error) -> IvError + '_ {
    move |_| IvError::new(ErrorCode::IndexFailed, path)
}

fn modified_ms(metadata: &fs::Metadata) -> i64 {
    metadata
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
}

/// `folder` with a trailing separator, for "everything below" prefix matches.
fn below(folder: &Path) -> String {
    let mut text = folder.display().to_string();
    if !text.ends_with(std::path::MAIN_SEPARATOR) {
        text.push(std::path::MAIN_SEPARATOR);
    }
    text
}

impl Index {
    /// Open (or create) the index at `file`.
    pub fn open(file: &Path) -> Result<Self, IvError> {
        if let Some(dir) = file.parent() {
            fs::create_dir_all(dir).map_err(|e| IvError::from_io(&e, dir))?;
        }
        let db = Connection::open(file).map_err(sql_error(file))?;
        Self::prepare(db, file)
    }

    /// An index in memory (tests).
    pub fn in_memory() -> Result<Self, IvError> {
        let db = Connection::open_in_memory().map_err(sql_error(Path::new(":memory:")))?;
        Self::prepare(db, Path::new(":memory:"))
    }

    fn prepare(db: Connection, file: &Path) -> Result<Self, IvError> {
        let failed = sql_error(file);
        db.execute_batch("PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL;")
            .map_err(&failed)?;
        let version: i64 = db
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .map_err(&failed)?;
        if version != SCHEMA {
            db.execute_batch(&format!(
                "DROP TABLE IF EXISTS images;
                 CREATE TABLE images (
                   path TEXT PRIMARY KEY,
                   folder TEXT NOT NULL,
                   size INTEGER NOT NULL,
                   modified INTEGER NOT NULL,
                   taken TEXT,
                   camera TEXT,
                   width INTEGER,
                   height INTEGER,
                   hash TEXT
                 );
                 CREATE INDEX images_folder ON images (folder);
                 CREATE INDEX images_hash ON images (hash);
                 PRAGMA user_version = {SCHEMA};"
            ))
            .map_err(&failed)?;
        }
        Ok(Self { db })
    }

    /// Index `root` (and its subfolders when `recursive`). `cancel` stops it between files; what
    /// is done stays done. `on_progress` is called from this thread.
    pub fn index(
        &mut self,
        root: &Path,
        recursive: bool,
        hashes: bool,
        cancel: &AtomicBool,
        mut on_progress: impl FnMut(Progress),
    ) -> Result<IndexSummary, IvError> {
        fs::read_dir(root).map_err(|e| IvError::from_io(&e, root))?;
        let failed = sql_error(root);
        let mut summary = IndexSummary::default();
        let mut seen: Vec<String> = Vec::new();
        let mut stack: Vec<PathBuf> = vec![root.to_path_buf()];
        let tx = self.db.transaction().map_err(&failed)?;
        {
            let mut known = tx
                .prepare("SELECT size, modified FROM images WHERE path = ?1")
                .map_err(&failed)?;
            let mut upsert = tx
                .prepare(
                    "INSERT INTO images (path, folder, size, modified, taken, camera, width, height, hash)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, NULL)
                     ON CONFLICT(path) DO UPDATE SET folder = ?2, size = ?3, modified = ?4,
                       taken = ?5, camera = ?6, width = ?7, height = ?8, hash = NULL",
                )
                .map_err(&failed)?;
            'walk: while let Some(dir) = stack.pop() {
                let Ok(items) = fs::read_dir(&dir) else {
                    summary.skipped_folders += 1;
                    continue;
                };
                let mut subfolders = Vec::new();
                for item in items.filter_map(Result::ok) {
                    if cancel.load(Ordering::Relaxed) {
                        summary.cancelled = true;
                        break 'walk;
                    }
                    let Ok(kind) = item.file_type() else { continue };
                    let path = item.path();
                    // Symlinked folders are not followed: no loops, nothing outside the root.
                    if kind.is_dir() {
                        if recursive {
                            subfolders.push(path);
                        }
                        continue;
                    }
                    if !kind.is_file() || format_of(&path).is_none() {
                        continue;
                    }
                    let Ok(metadata) = item.metadata() else {
                        continue;
                    };
                    let text = path.display().to_string();
                    let size = metadata.len();
                    let modified = modified_ms(&metadata);
                    let before: Option<(i64, i64)> = known
                        .query_row(params![text], |row| Ok((row.get(0)?, row.get(1)?)))
                        .optional()
                        .map_err(&failed)?;
                    let same = before
                        .is_some_and(|(s, m)| u64::try_from(s).ok() == Some(size) && m == modified);
                    if same {
                        summary.unchanged += 1;
                    } else {
                        let exif = read_exif(&path);
                        let exif = exif.as_ref();
                        upsert
                            .execute(params![
                                text,
                                dir.display().to_string(),
                                i64::try_from(size).unwrap_or(i64::MAX),
                                modified,
                                exif.and_then(|e| e.taken.clone()),
                                exif.and_then(|e| e.camera.clone()),
                                exif.and_then(|e| e.width),
                                exif.and_then(|e| e.height),
                            ])
                            .map_err(&failed)?;
                        if before.is_some() {
                            summary.changed += 1;
                        } else {
                            summary.added += 1;
                        }
                    }
                    seen.push(text);
                    summary.images += 1;
                    if summary.images % REPORT_EVERY == 0 {
                        on_progress(Progress::Files {
                            seen: summary.images,
                        });
                    }
                }
                subfolders.sort_by(|a, b| b.cmp(a));
                stack.extend(subfolders);
            }
            // Files gone from the walked folders leave the index (only after a complete walk:
            // a cancelled one has not seen everything).
            if !summary.cancelled {
                seen.sort_unstable();
                let mut listed = tx
                    .prepare(if recursive {
                        "SELECT path FROM images WHERE folder = ?1 OR substr(folder, 1, length(?2)) = ?2"
                    } else {
                        "SELECT path FROM images WHERE folder = ?1 AND ?2 = ?2"
                    })
                    .map_err(&failed)?;
                let gone: Vec<String> = listed
                    .query_map(params![root.display().to_string(), below(root)], |row| {
                        row.get(0)
                    })
                    .map_err(&failed)?
                    .filter_map(Result::ok)
                    .filter(|path: &String| seen.binary_search(path).is_err())
                    .collect();
                let mut delete = tx
                    .prepare("DELETE FROM images WHERE path = ?1")
                    .map_err(&failed)?;
                for path in &gone {
                    delete.execute(params![path]).map_err(&failed)?;
                }
                summary.removed = u64::try_from(gone.len()).unwrap_or(u64::MAX);
            }
        }
        tx.commit().map_err(&failed)?;
        on_progress(Progress::Files {
            seen: summary.images,
        });
        if hashes && !summary.cancelled {
            self.hash_pass(root, recursive, cancel, &mut summary, &mut on_progress)?;
        }
        Ok(summary)
    }

    /// The thumbnail pass: every image of `root` gets its thumbnail in `cache` (already cached
    /// ones cost a file lookup). Run after [`Index::index`]; stopped by `cancel`.
    pub fn thumbnails(
        &self,
        root: &Path,
        recursive: bool,
        cache: &crate::thumbs::ThumbCache,
        cancel: &AtomicBool,
        summary: &mut IndexSummary,
        mut on_progress: impl FnMut(Progress),
    ) -> Result<(), IvError> {
        let images = self.images(root, recursive)?;
        let total = u64::try_from(images.len()).unwrap_or(u64::MAX);
        for (done, image) in images.iter().enumerate() {
            if cancel.load(Ordering::Relaxed) {
                summary.cancelled = true;
                break;
            }
            if let Ok(true) = cache.ensure(Path::new(&image.path)) {
                summary.thumbnails += 1;
            }
            let done = u64::try_from(done + 1).unwrap_or(u64::MAX);
            if done % 50 == 0 || done == total {
                on_progress(Progress::Thumbnails { done, total });
            }
        }
        Ok(())
    }

    fn hash_pass(
        &mut self,
        root: &Path,
        recursive: bool,
        cancel: &AtomicBool,
        summary: &mut IndexSummary,
        on_progress: &mut impl FnMut(Progress),
    ) -> Result<(), IvError> {
        let failed = sql_error(root);
        let todo: Vec<String> = {
            let mut query = self
                .db
                .prepare(if recursive {
                    "SELECT path FROM images WHERE hash IS NULL AND (folder = ?1 OR substr(folder, 1, length(?2)) = ?2) ORDER BY path"
                } else {
                    "SELECT path FROM images WHERE hash IS NULL AND folder = ?1 AND ?2 = ?2 ORDER BY path"
                })
                .map_err(&failed)?;
            let rows = query
                .query_map(params![root.display().to_string(), below(root)], |row| {
                    row.get(0)
                })
                .map_err(&failed)?;
            rows.filter_map(Result::ok).collect()
        };
        let total = u64::try_from(todo.len()).unwrap_or(u64::MAX);
        let mut store = self
            .db
            .prepare("UPDATE images SET hash = ?2 WHERE path = ?1")
            .map_err(&failed)?;
        for (done, path) in todo.iter().enumerate() {
            if cancel.load(Ordering::Relaxed) {
                summary.cancelled = true;
                break;
            }
            if let Ok(hash) = hash_file(Path::new(path), cancel) {
                store.execute(params![path, hash]).map_err(&failed)?;
                summary.hashed += 1;
            }
            let done = u64::try_from(done + 1).unwrap_or(u64::MAX);
            if done % 50 == 0 || done == total {
                on_progress(Progress::Hashes { done, total });
            }
        }
        Ok(())
    }

    /// The images of `folder` (and below when `recursive`) in the index, by path.
    pub fn images(&self, folder: &Path, recursive: bool) -> Result<Vec<Indexed>, IvError> {
        let failed = sql_error(folder);
        let mut query = self
            .db
            .prepare(if recursive {
                "SELECT path, size, modified, taken, camera, width, height, hash FROM images
                 WHERE folder = ?1 OR substr(folder, 1, length(?2)) = ?2 ORDER BY path"
            } else {
                "SELECT path, size, modified, taken, camera, width, height, hash FROM images
                 WHERE folder = ?1 AND ?2 = ?2 ORDER BY path"
            })
            .map_err(&failed)?;
        let rows = query
            .query_map(
                params![folder.display().to_string(), below(folder)],
                |row| {
                    Ok(Indexed {
                        path: row.get(0)?,
                        size: u64::try_from(row.get::<_, i64>(1)?).unwrap_or(0),
                        modified: row.get(2)?,
                        taken: row.get(3)?,
                        camera: row.get(4)?,
                        width: row.get(5)?,
                        height: row.get(6)?,
                        hash: row.get(7)?,
                    })
                },
            )
            .map_err(&failed)?;
        Ok(rows.filter_map(Result::ok).collect())
    }
}

/// BLAKE3 of a file's content, hex; stops (error) when cancelled.
pub fn hash_file(path: &Path, cancel: &AtomicBool) -> Result<String, IvError> {
    use std::io::Read;
    let mut file = fs::File::open(path).map_err(|e| IvError::from_io(&e, path))?;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = vec![0u8; 1 << 20];
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(IvError::new(ErrorCode::Cancelled, path));
        }
        let n = file
            .read(&mut buffer)
            .map_err(|e| IvError::from_io(&e, path))?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }
    Ok(hasher.finalize().to_hex().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn testdata(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../testdata")
            .join(name)
    }

    /// testdata photos copied into a temporary tree: root, root/2026, root/2026/summer.
    fn archive() -> tempfile::TempDir {
        let temp = tempfile::tempdir().expect("temp");
        let root = temp.path();
        fs::create_dir_all(root.join("2026/summer")).expect("mkdir");
        for (name, to) in [
            ("orientation-1.jpg", "a.jpg"),
            ("red.png", "b.png"),
            ("notes.txt", "notes.txt"),
            ("orientation-6.jpg", "2026/c.jpg"),
            ("thumb-6.jpg", "2026/summer/d.jpg"),
            ("orientation-1.jpg", "2026/summer/copy-of-a.jpg"),
        ] {
            fs::copy(testdata(name), root.join(to)).expect("copy");
        }
        temp
    }

    fn run(index: &mut Index, root: &Path, recursive: bool) -> IndexSummary {
        index
            .index(root, recursive, true, &AtomicBool::new(false), |_| {})
            .expect("index")
    }

    #[test]
    fn indexes_images_with_exif_and_hashes() {
        let temp = archive();
        let mut index = Index::in_memory().expect("db");
        let summary = run(&mut index, temp.path(), true);
        assert_eq!((summary.images, summary.added, summary.hashed), (5, 5, 5));
        let all = index.images(temp.path(), true).expect("images");
        assert_eq!(all.len(), 5);
        let a = all.iter().find(|i| i.path.ends_with("a.jpg")).expect("a");
        let copy = all
            .iter()
            .find(|i| i.path.ends_with("copy-of-a.jpg"))
            .expect("copy");
        assert_eq!(a.hash, copy.hash, "same bytes, same hash");
        assert_ne!(
            a.hash,
            all.iter()
                .find(|i| i.path.ends_with("c.jpg"))
                .expect("c")
                .hash
        );
        assert_eq!(a.hash.as_deref().map(str::len), Some(64));
        // Only the top folder without subfolders.
        assert_eq!(index.images(temp.path(), false).expect("top").len(), 2);
    }

    #[test]
    fn opening_again_skips_unchanged_and_follows_changes() {
        let temp = archive();
        let root = temp.path();
        let mut index = Index::in_memory().expect("db");
        run(&mut index, root, true);
        let again = run(&mut index, root, true);
        assert_eq!(
            (again.unchanged, again.added, again.changed, again.hashed),
            (5, 0, 0, 0)
        );
        // One file changes, one vanishes, one appears.
        fs::copy(testdata("blue.bmp"), root.join("b.png")).expect("change");
        fs::remove_file(root.join("2026/c.jpg")).expect("remove");
        fs::copy(testdata("green.gif"), root.join("2026/summer/e.gif")).expect("add");
        let after = run(&mut index, root, true);
        assert_eq!(
            (
                after.changed,
                after.removed,
                after.added,
                after.hashed,
                after.images
            ),
            (1, 1, 1, 2, 5)
        );
        assert!(index
            .images(root, true)
            .expect("images")
            .iter()
            .all(|i| !i.path.ends_with("c.jpg")));
    }

    #[test]
    fn indexing_never_changes_the_photos() {
        let temp = archive();
        let before: Vec<Vec<u8>> = ["a.jpg", "2026/c.jpg"]
            .iter()
            .map(|n| fs::read(temp.path().join(n)).expect("read"))
            .collect();
        run(&mut Index::in_memory().expect("db"), temp.path(), true);
        for (name, bytes) in ["a.jpg", "2026/c.jpg"].iter().zip(before) {
            assert_eq!(
                fs::read(temp.path().join(name)).expect("read"),
                bytes,
                "{name}"
            );
        }
    }

    #[test]
    fn cancel_keeps_what_was_done_and_removes_nothing() {
        let temp = archive();
        let root = temp.path();
        let mut index = Index::in_memory().expect("db");
        run(&mut index, root, true);
        fs::remove_file(root.join("a.jpg")).expect("remove");
        let cancelled = index
            .index(root, true, true, &AtomicBool::new(true), |_| {})
            .expect("index");
        assert!(cancelled.cancelled);
        assert_eq!(
            cancelled.removed, 0,
            "a cancelled walk has not seen everything"
        );
        assert_eq!(index.images(root, true).expect("images").len(), 5);
        assert!(hash_file(&root.join("b.png"), &AtomicBool::new(true)).is_err());
    }

    #[test]
    fn index_file_survives_reopening() {
        let temp = archive();
        let db = temp.path().join("cache/index.sqlite");
        run(&mut Index::open(&db).expect("db"), temp.path(), true);
        let mut reopened = Index::open(&db).expect("reopen");
        assert_eq!(run(&mut reopened, temp.path(), true).unchanged, 5);
    }

    #[test]
    fn thumbnails_go_into_the_shared_cache_once() {
        let temp = archive();
        let mut index = Index::in_memory().expect("db");
        let mut summary = run(&mut index, temp.path(), true);
        let cache =
            crate::thumbs::ThumbCache::new(&temp.path().join("cache"), crate::thumbs::CACHE_LIMIT);
        index
            .thumbnails(
                temp.path(),
                true,
                &cache,
                &AtomicBool::new(false),
                &mut summary,
                |_| {},
            )
            .expect("thumbs");
        assert_eq!(summary.thumbnails, 5);
        assert_eq!(fs::read_dir(cache.dir()).expect("dir").count(), 5);
        let mut again = IndexSummary::default();
        index
            .thumbnails(
                temp.path(),
                true,
                &cache,
                &AtomicBool::new(false),
                &mut again,
                |_| {},
            )
            .expect("again");
        assert_eq!(again.thumbnails, 0, "already in the cache");
        // The strip asks the same cache and gets the same thumbnail.
        let strip = cache.get(&temp.path().join("a.jpg")).expect("strip");
        assert_eq!(
            strip,
            crate::thumbs::make(&temp.path().join("a.jpg")).expect("make")
        );
    }

    #[test]
    fn missing_folder_is_an_error() {
        let temp = tempfile::tempdir().expect("temp");
        let mut index = Index::in_memory().expect("db");
        let error = index
            .index(
                &temp.path().join("missing"),
                true,
                true,
                &AtomicBool::new(false),
                |_| {},
            )
            .expect_err("missing");
        assert_eq!(error.code, ErrorCode::NotFound);
    }

    /// 50 000 synthetic files (IV-13): `cargo test -p iv-core --release -- --ignored --nocapture
    /// synthetic_archive`. Files are tiny: this measures the index, not the disk.
    #[test]
    #[ignore]
    fn synthetic_archive_of_50_000() {
        let temp = tempfile::tempdir().expect("temp");
        let pixel = fs::read(testdata("red.png")).expect("png");
        for folder in 0..100 {
            let dir = temp.path().join(format!("{folder:03}"));
            fs::create_dir(&dir).expect("mkdir");
            for i in 0..500 {
                fs::write(dir.join(format!("IMG_{i:04}.png")), &pixel).expect("write");
            }
        }
        let mut index = Index::open(&temp.path().join("index.sqlite")).expect("db");
        let mut reports = 0;
        let start = std::time::Instant::now();
        let first = index
            .index(temp.path(), true, true, &AtomicBool::new(false), |_| {
                reports += 1
            })
            .expect("index");
        println!(
            "first: {:?}, {} images, {reports} progress reports",
            start.elapsed(),
            first.images
        );
        let start = std::time::Instant::now();
        let again = index
            .index(temp.path(), true, true, &AtomicBool::new(false), |_| {})
            .expect("again");
        println!(
            "again: {:?}, {} unchanged",
            start.elapsed(),
            again.unchanged
        );
        assert_eq!((first.images, again.unchanged), (50_000, 50_000));
    }
}
