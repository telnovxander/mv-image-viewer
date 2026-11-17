//! Thin Tauri layer of Spiegelmond: commands that call `iv-core`, the single window, settings.

use std::path::PathBuf;
use std::sync::Mutex;

use iv_core::folder::{folder_of, list_images, FolderEntry, SortOrder};
use iv_core::orientation::{self, JournalEntry, OrientationPlan};
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
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(InitialPath(Mutex::new(first)))
        .invoke_handler(tauri::generate_handler![
            initial_path,
            open_image,
            list_folder,
            exif_of,
            open_map,
            plan_rotation,
            apply_rotation,
            undo_rotation,
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
