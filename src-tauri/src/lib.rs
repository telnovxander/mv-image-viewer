//! Thin Tauri layer of Spiegelmond: commands that call `iv-core`, the single window, settings.

use std::path::PathBuf;
use std::sync::Mutex;

use iv_core::folder::{folder_of, list_images, FolderEntry, SortOrder};
use iv_core::orientation::{self, JournalEntry, OrientationPlan};
use iv_core::recycle::{self, Trashed};
use iv_core::{
    describe, path_from_args, read_exif, settings, ErrorCode, ExifInfo, ImageFile, IvError,
};
use tauri::{Emitter, Manager};
use tauri_plugin_opener::OpenerExt;

/// The file named on the command line of the first start, taken once by the interface.
#[derive(Default)]
struct InitialPath(Mutex<Option<PathBuf>>);

/// `%XX` escapes of a URL path back to text (the window encodes file paths that way).
fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = |b: u8| (b as char).to_digit(16);
        match (
            bytes[i],
            bytes.get(i + 1).copied().and_then(hex),
            bytes.get(i + 2).copied().and_then(hex),
        ) {
            (b'%', Some(high), Some(low)) => {
                out.push(u8::try_from(high * 16 + low).unwrap_or(b'?'));
                i += 3;
            }
            (byte, _, _) => {
                out.push(byte);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// `ivimg://localhost/<path>` (Windows: `http://ivimg.localhost/<path>`): TIFF and RAW previews
/// rendered by `iv-core` on a background thread (IV-05). A failure answers 422, and the window
/// shows its placeholder.
fn serve_decoded(request: tauri::http::Request<Vec<u8>>, responder: tauri::UriSchemeResponder) {
    let path = percent_decode(request.uri().path().trim_start_matches('/'));
    std::thread::spawn(move || {
        let response = match iv_core::decode::render(std::path::Path::new(&path)) {
            Ok(rendered) => tauri::http::Response::builder()
                .header("Content-Type", rendered.mime)
                .header("Cache-Control", "no-store")
                .body(rendered.bytes),
            Err(_) => tauri::http::Response::builder()
                .status(422)
                .body(Vec::new()),
        };
        responder.respond(response.unwrap_or_else(|_| tauri::http::Response::new(Vec::new())));
    });
}

/// Whether the HEIC decoder (libheif next to the program) could be loaded: the window then tells a
/// missing decoder from a damaged HEIC file.
#[tauri::command]
fn heif_available() -> bool {
    iv_core::heif::available()
}

/// A thumbnail request waiting for a worker.
type ThumbJob = (PathBuf, tauri::UriSchemeResponder);

/// Thumbnails of the film strip (IV-08) are built by a few workers, the **latest** request first:
/// the strip asks for what is on screen last, so visible thumbnails come before ones scrolled
/// past. The disk cache lives with the settings (`<data>/cache/thumbs`, shared with Vollmond).
struct ThumbQueue {
    jobs: Mutex<Vec<ThumbJob>>,
    ready: std::sync::Condvar,
    cache: Option<iv_core::thumbs::ThumbCache>,
}

const THUMB_WORKERS: usize = 3;
const THUMB_BACKLOG: usize = 300;

fn thumb_queue() -> &'static ThumbQueue {
    static QUEUE: std::sync::OnceLock<&'static ThumbQueue> = std::sync::OnceLock::new();
    QUEUE.get_or_init(|| {
        let cache = settings::data_dir().map(|dir| {
            iv_core::thumbs::ThumbCache::new(
                &dir.join("cache").join("thumbs"),
                iv_core::thumbs::CACHE_LIMIT,
            )
        });
        let queue: &'static ThumbQueue = Box::leak(Box::new(ThumbQueue {
            jobs: Mutex::new(Vec::new()),
            ready: std::sync::Condvar::new(),
            cache,
        }));
        if let Some(cache) = queue.cache.clone() {
            // Keep the disk cache under its limit (once per start, in the background).
            std::thread::spawn(move || cache.trim());
        }
        for _ in 0..THUMB_WORKERS {
            std::thread::spawn(move || thumb_worker(queue));
        }
        queue
    })
}

fn thumb_worker(queue: &'static ThumbQueue) {
    loop {
        let job = {
            let Ok(mut jobs) = queue.jobs.lock() else {
                return;
            };
            loop {
                if let Some(job) = jobs.pop() {
                    break job;
                }
                jobs = match queue.ready.wait(jobs) {
                    Ok(jobs) => jobs,
                    Err(_) => return,
                };
            }
        };
        let (path, responder) = job;
        let thumb = match &queue.cache {
            Some(cache) => cache.get(&path),
            None => iv_core::thumbs::make(&path),
        };
        let response = match thumb {
            Ok(jpeg) => tauri::http::Response::builder()
                .header("Content-Type", "image/jpeg")
                .header("Cache-Control", "max-age=3600")
                .body(jpeg),
            Err(_) => tauri::http::Response::builder()
                .status(422)
                .body(Vec::new()),
        };
        responder.respond(response.unwrap_or_else(|_| tauri::http::Response::new(Vec::new())));
    }
}

/// `ivthumb://localhost/<path>` (Windows: `http://ivthumb.localhost/<path>`): a thumbnail.
fn serve_thumb(request: tauri::http::Request<Vec<u8>>, responder: tauri::UriSchemeResponder) {
    let path = PathBuf::from(percent_decode(request.uri().path().trim_start_matches('/')));
    let queue = thumb_queue();
    if let Ok(mut jobs) = queue.jobs.lock() {
        // Fast scrolling through 10 000 photos: the oldest requests (long off screen) are given
        // up; the strip asks again when they come back into view.
        if jobs.len() >= THUMB_BACKLOG {
            let (_, old) = jobs.remove(0);
            let busy = tauri::http::Response::builder()
                .status(503)
                .body(Vec::new());
            old.respond(busy.unwrap_or_else(|_| tauri::http::Response::new(Vec::new())));
        }
        jobs.push((path, responder));
        queue.ready.notify_one();
    }
}

/// Event with the path of a file opened by a second start (`iv path`, a double-click).
const OPEN_PATH_EVENT: &str = "open-path";

#[tauri::command]
fn initial_path(state: tauri::State<InitialPath>) -> Option<String> {
    state
        .0
        .lock()
        .ok()
        .and_then(|mut path| path.take())
        .map(|path| path.display().to_string())
}

#[tauri::command]
fn open_image(path: String) -> Result<ImageFile, IvError> {
    describe(&PathBuf::from(path))
}

/// The images of the folder of `file`, sorted, read on a background thread: the first image is
/// already on screen while the list is built.
#[tauri::command]
async fn list_folder(file: String, order: SortOrder) -> Result<Vec<FolderEntry>, IvError> {
    let path = PathBuf::from(&file);
    tauri::async_runtime::spawn_blocking(move || list_images(&folder_of(&path)?, order))
        .await
        .map_err(|_| IvError::new(ErrorCode::Other, std::path::Path::new(&file)))?
}

/// EXIF of an image for the information panel; `None` without EXIF. Reads only.
#[tauri::command]
async fn exif_of(path: String) -> Option<ExifInfo> {
    tauri::async_runtime::spawn_blocking(move || read_exif(std::path::Path::new(&path)))
        .await
        .ok()
        .flatten()
}

/// Open the place of a photo on OpenStreetMap in the browser; only on the user's click.
#[tauri::command]
fn open_map(app: tauri::AppHandle, latitude: f64, longitude: f64) -> Result<(), IvError> {
    let url = iv_core::exif::map_url(latitude, longitude);
    app.opener()
        .open_url(&url, None::<&str>)
        .map_err(|_| IvError::new(ErrorCode::Other, std::path::Path::new(&url)))
}

/// The journal of metadata changes lives with the settings, outside any photo folder.
fn journal_dir() -> Result<PathBuf, IvError> {
    settings::data_dir()
        .map(|dir| dir.join("journal"))
        .ok_or_else(|| {
            IvError::new(
                ErrorCode::NotFound,
                std::path::Path::new(settings::APP_FOLDER),
            )
        })
}

/// Preview of writing the view rotation into the file: nothing is changed.
#[tauri::command]
fn plan_rotation(path: String, degrees: u16) -> Result<OrientationPlan, IvError> {
    orientation::plan_rotation(std::path::Path::new(&path), degrees)
}

/// Write a confirmed plan (journal first, atomic, checked byte by byte).
#[tauri::command]
fn apply_rotation(plan: OrientationPlan) -> Result<JournalEntry, IvError> {
    orientation::apply(&plan, &journal_dir()?)
}

#[tauri::command]
fn undo_rotation(entry: JournalEntry) -> Result<JournalEntry, IvError> {
    orientation::undo(&entry, &journal_dir()?)
}

/// Delete: the file goes to the Recycle Bin, never past it (IV/PROJECT.md, safety points 1–3).
#[tauri::command]
async fn trash_file(path: String) -> Result<Trashed, IvError> {
    let file = PathBuf::from(&path);
    tauri::async_runtime::spawn_blocking(move || recycle::move_to_trash(&file))
        .await
        .map_err(|_| IvError::new(ErrorCode::Other, std::path::Path::new(&path)))?
}

/// Ctrl+Z after Delete: the file comes back from the Recycle Bin (safety point 4).
#[tauri::command]
async fn restore_file(trashed: Trashed) -> Result<(), IvError> {
    let path = trashed.path.clone();
    tauri::async_runtime::spawn_blocking(move || recycle::restore(&trashed))
        .await
        .map_err(|_| IvError::new(ErrorCode::Other, std::path::Path::new(&path)))?
}

/// Ctrl+C: the image as shown (EXIF orientation and view rotation) into the clipboard.
#[tauri::command]
async fn copy_image(path: String, degrees: u16) -> Result<(), IvError> {
    let file = PathBuf::from(&path);
    tauri::async_runtime::spawn_blocking(move || {
        let failed = || IvError::new(ErrorCode::CopyFailed, &file);
        let pixels = iv_core::copy::pixels(&file, degrees)?;
        let (Ok(width), Ok(height)) = (
            usize::try_from(pixels.width),
            usize::try_from(pixels.height),
        ) else {
            return Err(failed());
        };
        let mut clipboard = arboard::Clipboard::new().map_err(|_| failed())?;
        clipboard
            .set_image(arboard::ImageData {
                width,
                height,
                bytes: pixels.rgba.into(),
            })
            .map_err(|_| failed())
    })
    .await
    .map_err(|_| IvError::new(ErrorCode::Other, std::path::Path::new(&path)))?
}

/// Ctrl+Shift+C: the full path of the file as text.
#[tauri::command]
fn copy_path(path: String) -> Result<(), IvError> {
    let failed = || IvError::new(ErrorCode::CopyFailed, std::path::Path::new(&path));
    arboard::Clipboard::new()
        .and_then(|mut clipboard| clipboard.set_text(path.clone()))
        .map_err(|_| failed())
}

/// "Show in Explorer": the folder opens with the file selected.
#[tauri::command]
fn reveal_file(app: tauri::AppHandle, path: String) -> Result<(), IvError> {
    app.opener()
        .reveal_item_in_dir(&path)
        .map_err(|_| IvError::new(ErrorCode::Other, std::path::Path::new(&path)))
}

/// "Open with": Windows' dialog to choose a program, owned by Spiegelmond's window.
#[tauri::command]
async fn open_with(window: tauri::WebviewWindow, path: String) -> Result<(), IvError> {
    #[cfg(windows)]
    let owner = window.hwnd().map(|h| h.0 as isize).unwrap_or(0);
    #[cfg(not(windows))]
    let owner = {
        let _ = &window;
        0
    };
    let file = PathBuf::from(&path);
    tauri::async_runtime::spawn_blocking(move || iv_core::open_with::choose_program(&file, owner))
        .await
        .map_err(|_| IvError::new(ErrorCode::Other, std::path::Path::new(&path)))?
}

/// Long Vollmond jobs that can be stopped: their cancel flags by id (IV-13).
#[derive(Default)]
struct Jobs(std::sync::Arc<Mutex<JobMap>>);

type JobMap = std::collections::HashMap<u32, std::sync::Arc<std::sync::atomic::AtomicBool>>;

/// Events of an indexing job.
#[derive(Clone, serde::Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
enum IndexEvent {
    Progress {
        progress: iv_core::vollmond::index::Progress,
    },
    Done {
        summary: iv_core::vollmond::index::IndexSummary,
    },
    Error {
        error: IvError,
    },
}

/// Where Vollmond keeps its index (next to the thumbnails, `<data>/cache`) and its journal.
fn vollmond_dirs() -> Result<(PathBuf, PathBuf), IvError> {
    let data = settings::data_dir().ok_or_else(|| {
        IvError::new(
            ErrorCode::NotFound,
            std::path::Path::new(settings::APP_FOLDER),
        )
    })?;
    Ok((data.join("cache"), data.join("journal")))
}

/// Index a folder of the archive in the background; stopped with `cancel_job` and the same id.
#[tauri::command]
fn index_folder(
    id: u32,
    root: String,
    recursive: bool,
    on_event: tauri::ipc::Channel<IndexEvent>,
    jobs: tauri::State<'_, Jobs>,
) {
    let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    if let Ok(mut map) = jobs.0.lock() {
        map.insert(id, cancel.clone());
    }
    let registry = jobs.0.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let path = PathBuf::from(&root);
        let result = vollmond_dirs().and_then(|(cache, _)| {
            let mut index = iv_core::vollmond::index::Index::open(&cache.join("index.sqlite"))?;
            let mut summary = index.index(&path, recursive, true, &cancel, |progress| {
                let _ = on_event.send(IndexEvent::Progress { progress });
            })?;
            if !summary.cancelled {
                // The same cache as the film strip (thumb_queue): built once for both.
                let thumbs = iv_core::thumbs::ThumbCache::new(
                    &cache.join("thumbs"),
                    iv_core::thumbs::CACHE_LIMIT,
                );
                index.thumbnails(
                    &path,
                    recursive,
                    &thumbs,
                    &cancel,
                    &mut summary,
                    |progress| {
                        let _ = on_event.send(IndexEvent::Progress { progress });
                    },
                )?;
            }
            Ok(summary)
        });
        let _ = on_event.send(match result {
            Ok(summary) => IndexEvent::Done { summary },
            Err(error) => IndexEvent::Error { error },
        });
        if let Ok(mut map) = registry.lock() {
            map.remove(&id);
        }
    });
}

#[tauri::command]
fn cancel_job(id: u32, jobs: tauri::State<'_, Jobs>) {
    if let Ok(mut map) = jobs.0.lock() {
        if let Some(cancel) = map.remove(&id) {
            cancel.store(true, std::sync::atomic::Ordering::Relaxed);
        }
    }
}

/// Today in the local zone, for "a date in the future" (IV-14, point 13).
fn today() -> iv_core::vollmond::dates::Stamp {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX));
    // 2100-01-01 if the clock cannot be read: nothing counts as "in the future" then.
    iv_core::vollmond::dates_tool::utc_to_local(now).unwrap_or(iv_core::vollmond::dates::Stamp {
        year: 2100,
        month: 1,
        day: 1,
        hour: 0,
        minute: 0,
        second: 0,
    })
}

fn indexed(
    folder: &str,
    recursive: bool,
) -> Result<Vec<iv_core::vollmond::index::Indexed>, IvError> {
    let (cache, _) = vollmond_dirs()?;
    let index = iv_core::vollmond::index::Index::open(&cache.join("index.sqlite"))?;
    index.images(std::path::Path::new(folder), recursive)
}

/// The dates table of an indexed folder (IV-14). Reads only.
#[tauri::command]
async fn dates_rows(
    folder: String,
    recursive: bool,
) -> Result<Vec<iv_core::vollmond::dates::DateRow>, IvError> {
    tauri::async_runtime::spawn_blocking(move || {
        Ok(iv_core::vollmond::dates_tool::rows(
            &indexed(&folder, recursive)?,
            today(),
        ))
    })
    .await
    .map_err(|_| IvError::new(ErrorCode::Other, std::path::Path::new("dates")))?
}

/// "Prepare for the phone" (IV-16): what would sort wrong in a phone gallery. Reads only.
#[tauri::command]
async fn transfer_report(
    folder: String,
    recursive: bool,
) -> Result<iv_core::vollmond::dates_tool::TransferReport, IvError> {
    tauri::async_runtime::spawn_blocking(move || {
        let rows = iv_core::vollmond::dates_tool::rows(&indexed(&folder, recursive)?, today());
        Ok(iv_core::vollmond::dates_tool::transfer_report(&rows))
    })
    .await
    .map_err(|_| IvError::new(ErrorCode::Other, std::path::Path::new("dates")))?
}

/// Events of a duplicate search (IV-17).
#[derive(Clone, serde::Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
enum DuplicatesEvent {
    Progress {
        progress: iv_core::vollmond::duplicates::Progress,
    },
    Done {
        groups: Vec<iv_core::vollmond::duplicates::Group>,
        cancelled: bool,
    },
    Error {
        error: IvError,
    },
}

/// Find exact duplicates in an indexed folder, in the background; `cancel_job` stops it.
#[tauri::command]
fn find_duplicates(
    id: u32,
    folder: String,
    recursive: bool,
    on_event: tauri::ipc::Channel<DuplicatesEvent>,
    jobs: tauri::State<'_, Jobs>,
) {
    let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    if let Ok(mut map) = jobs.0.lock() {
        map.insert(id, cancel.clone());
    }
    let registry = jobs.0.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let event = match indexed(&folder, recursive) {
            Ok(images) => {
                let groups = iv_core::vollmond::duplicates::find(&images, &cancel, |progress| {
                    let _ = on_event.send(DuplicatesEvent::Progress { progress });
                });
                DuplicatesEvent::Done {
                    groups,
                    cancelled: cancel.load(std::sync::atomic::Ordering::Relaxed),
                }
            }
            Err(error) => DuplicatesEvent::Error { error },
        };
        let _ = on_event.send(event);
        if let Ok(mut map) = registry.lock() {
            map.remove(&id);
        }
    });
}

/// Events of a search for similar photos (IV-18).
#[derive(Clone, serde::Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
enum SimilarEvent {
    Progress {
        progress: iv_core::vollmond::similar::Progress,
    },
    Done {
        groups: Vec<iv_core::vollmond::similar::SimilarGroup>,
        cancelled: bool,
    },
    Error {
        error: IvError,
    },
}

/// Find similar photos in an indexed folder, in the background; `cancel_job` stops it.
#[tauri::command]
fn find_similar(
    id: u32,
    folder: String,
    recursive: bool,
    threshold: u32,
    on_event: tauri::ipc::Channel<SimilarEvent>,
    jobs: tauri::State<'_, Jobs>,
) {
    let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    if let Ok(mut map) = jobs.0.lock() {
        map.insert(id, cancel.clone());
    }
    let registry = jobs.0.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let result = vollmond_dirs().and_then(|(cache, _)| {
            let index = iv_core::vollmond::index::Index::open(&cache.join("index.sqlite"))?;
            let images = index.images(std::path::Path::new(&folder), recursive)?;
            let thumbs = iv_core::thumbs::ThumbCache::new(
                &cache.join("thumbs"),
                iv_core::thumbs::CACHE_LIMIT,
            );
            Ok(iv_core::vollmond::similar::find(
                &index,
                &images,
                threshold,
                Some(&thumbs),
                &cancel,
                |progress| {
                    let _ = on_event.send(SimilarEvent::Progress { progress });
                },
            ))
        });
        let _ = on_event.send(match result {
            Ok(groups) => SimilarEvent::Done {
                groups,
                cancelled: cancel.load(std::sync::atomic::Ordering::Relaxed),
            },
            Err(error) => SimilarEvent::Error { error },
        });
        if let Ok(mut map) = registry.lock() {
            map.remove(&id);
        }
    });
}

/// The preview of a batch (IV-19): every image of the indexed folder, its copy and its size.
#[derive(serde::Serialize)]
struct BatchPreview {
    plan: iv_core::vollmond::plan::Plan,
    summary: iv_core::vollmond::plan::PlanSummary,
    planned: Vec<iv_core::vollmond::batch::Planned>,
}

#[tauri::command]
async fn batch_preview(
    folder: String,
    recursive: bool,
    steps: Vec<iv_core::vollmond::batch::Step>,
    out_dir: String,
) -> Result<BatchPreview, IvError> {
    tauri::async_runtime::spawn_blocking(move || {
        let sources: Vec<PathBuf> = indexed(&folder, recursive)?
            .into_iter()
            .map(|i| PathBuf::from(i.path))
            .filter(|p| !iv_core::vollmond::video::is_video(p))
            .collect();
        let (plan, planned) =
            iv_core::vollmond::batch::plan(&sources, &steps, std::path::Path::new(&out_dir))?;
        let summary = plan.summary();
        Ok(BatchPreview {
            plan,
            summary,
            planned,
        })
    })
    .await
    .map_err(|_| IvError::new(ErrorCode::Other, std::path::Path::new("batch")))?
}

/// The preview of deleting the chosen duplicates (refused for a whole group or a protected folder).
#[tauri::command]
fn duplicates_preview(
    groups: Vec<iv_core::vollmond::duplicates::Group>,
    delete: Vec<Vec<String>>,
    protected: Vec<String>,
) -> Result<Preview, IvError> {
    let protected: Vec<PathBuf> = protected.into_iter().map(PathBuf::from).collect();
    let plan = iv_core::vollmond::duplicates::plan(&groups, &delete, &protected)?;
    let summary = plan.summary();
    Ok(Preview { plan, summary })
}

/// The preview of fixing the dates: the plan and its totals; nothing is changed.
#[derive(serde::Serialize)]
struct Preview {
    plan: iv_core::vollmond::plan::Plan,
    summary: iv_core::vollmond::plan::PlanSummary,
}

#[tauri::command]
async fn dates_preview(
    folder: String,
    recursive: bool,
    with_suspicious: bool,
) -> Result<Preview, IvError> {
    tauri::async_runtime::spawn_blocking(move || {
        let rows = iv_core::vollmond::dates_tool::rows(&indexed(&folder, recursive)?, today());
        let plan = iv_core::vollmond::dates_tool::plan(&rows, with_suspicious);
        let summary = plan.summary();
        Ok(Preview { plan, summary })
    })
    .await
    .map_err(|_| IvError::new(ErrorCode::Other, std::path::Path::new("dates")))?
}

/// The preview of setting dates by hand (IV-15) for `paths`: their current dates are read from
/// the disk here; nothing is changed.
#[tauri::command]
async fn manual_dates_preview(
    paths: Vec<String>,
    how: iv_core::vollmond::manual_dates::Manual,
    with_suspicious: bool,
) -> Result<Preview, IvError> {
    tauri::async_runtime::spawn_blocking(move || {
        let chosen = paths
            .iter()
            .map(|p| iv_core::vollmond::dates_tool::current(std::path::Path::new(p)))
            .collect::<Result<Vec<_>, _>>()?;
        let mut plan = iv_core::vollmond::manual_dates::plan(&chosen, &how, today());
        if !with_suspicious {
            plan.changes.retain(|c| !c.suspicious);
        }
        let summary = plan.summary();
        Ok(Preview { plan, summary })
    })
    .await
    .map_err(|_| IvError::new(ErrorCode::Other, std::path::Path::new("dates")))?
}

/// Apply one confirmed batch of a plan (journal first, the first failure stops it).
#[tauri::command]
async fn apply_batch(
    plan: iv_core::vollmond::plan::Plan,
    start: usize,
    end: usize,
    undoes: Option<u64>,
) -> Result<iv_core::vollmond::plan::ApplyReport, IvError> {
    tauri::async_runtime::spawn_blocking(move || {
        let (_, journal) = vollmond_dirs()?;
        let journal = iv_core::vollmond::plan::Journal::new(&journal);
        let apply_one = match plan.tool.as_str() {
            iv_core::vollmond::dates_tool::TOOL => iv_core::vollmond::dates_tool::apply_change,
            iv_core::vollmond::duplicates::TOOL => iv_core::vollmond::duplicates::apply_change,
            iv_core::vollmond::batch::TOOL => iv_core::vollmond::batch::apply_journalled,
            _ => {
                return Err(IvError::new(
                    ErrorCode::Unsupported,
                    std::path::Path::new(&plan.tool),
                ))
            }
        };
        let cancel = std::sync::atomic::AtomicBool::new(false);
        iv_core::vollmond::plan::apply(&plan, start..end, &journal, undoes, &cancel, apply_one)
    })
    .await
    .map_err(|_| IvError::new(ErrorCode::Other, std::path::Path::new("apply")))?
}

/// The plan that undoes operation `op` (applied with `apply_batch`, `undoes: op`).
#[tauri::command]
fn undo_plan(op: u64) -> Result<iv_core::vollmond::plan::Plan, IvError> {
    let (_, journal) = vollmond_dirs()?;
    iv_core::vollmond::plan::undo_plan(&iv_core::vollmond::plan::Journal::new(&journal), op)
        .ok_or_else(|| IvError::new(ErrorCode::NotFound, std::path::Path::new("journal")))
}

/// The journal of Vollmond operations, newest first (IV-13).
#[tauri::command]
fn vollmond_operations() -> Result<Vec<iv_core::vollmond::plan::Operation>, IvError> {
    let (_, journal) = vollmond_dirs()?;
    Ok(iv_core::vollmond::plan::Journal::new(&journal).operations())
}

/// The folder with the settings, the journal and the cache, shown in "About" (IV-20).
#[tauri::command]
fn data_folder() -> Option<String> {
    settings::data_dir().map(|dir| dir.to_string_lossy().into_owned())
}

#[tauri::command]
fn load_settings() -> Result<Option<String>, IvError> {
    match settings::data_dir() {
        Some(dir) => settings::load(&dir),
        None => Ok(None),
    }
}

#[tauri::command]
fn save_settings(text: String) -> Result<(), IvError> {
    match settings::data_dir() {
        Some(dir) => settings::save(&dir, &text),
        None => Err(IvError::new(
            ErrorCode::NotFound,
            std::path::Path::new(settings::APP_FOLDER),
        )),
    }
}

pub fn run() {
    let cwd = std::env::current_dir().unwrap_or_default();
    let args: Vec<String> = std::env::args().collect();
    let first = path_from_args(&args, &cwd);

    let result = tauri::Builder::default()
        // Must be the first plugin: a second start hands its path to this window and exits.
        .plugin(tauri_plugin_single_instance::init(|app, argv, cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
            if let Some(path) = path_from_args(&argv, &PathBuf::from(cwd)) {
                let _ = app.emit(OPEN_PATH_EVENT, path.display().to_string());
            }
        }))
        .register_asynchronous_uri_scheme_protocol("ivimg", |_context, request, responder| {
            serve_decoded(request, responder)
        })
        .register_asynchronous_uri_scheme_protocol("ivthumb", |_context, request, responder| {
            serve_thumb(request, responder)
        })
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(InitialPath(Mutex::new(first)))
        .manage(Jobs::default())
        .invoke_handler(tauri::generate_handler![
            data_folder,
            initial_path,
            open_image,
            list_folder,
            exif_of,
            heif_available,
            open_map,
            plan_rotation,
            apply_rotation,
            undo_rotation,
            trash_file,
            restore_file,
            copy_image,
            copy_path,
            reveal_file,
            open_with,
            index_folder,
            cancel_job,
            vollmond_operations,
            dates_rows,
            dates_preview,
            transfer_report,
            find_duplicates,
            duplicates_preview,
            find_similar,
            batch_preview,
            manual_dates_preview,
            apply_batch,
            undo_plan,
            load_settings,
            save_settings
        ])
        .run(tauri::generate_context!());
    if let Err(error) = result {
        eprintln!("Spiegelmond failed to start: {error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::percent_decode;

    #[test]
    fn decodes_paths_from_urls() {
        assert_eq!(
            percent_decode("C%3A%5CFotos%5CIMG%201.dng"),
            "C:\\Fotos\\IMG 1.dng"
        );
        assert_eq!(percent_decode("%D0%A4%D0%BE%D1%82%D0%BE.tif"), "Фото.tif");
        assert_eq!(percent_decode("100%"), "100%");
    }
}
