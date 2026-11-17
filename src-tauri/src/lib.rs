//! Thin Tauri layer of Spiegelmond: commands that call `iv-core`, the single window, settings.

use std::path::PathBuf;
use std::sync::Mutex;

use iv_core::folder::{folder_of, list_images, FolderEntry, SortOrder};
use iv_core::{describe, path_from_args, settings, ErrorCode, ImageFile, IvError};
use tauri::{Emitter, Manager};

/// The file named on the command line of the first start, taken once by the interface.
#[derive(Default)]
struct InitialPath(Mutex<Option<PathBuf>>);

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
        .plugin(tauri_plugin_dialog::init())
        .manage(InitialPath(Mutex::new(first)))
        .invoke_handler(tauri::generate_handler![
            initial_path,
            open_image,
            list_folder,
            load_settings,
            save_settings
        ])
        .run(tauri::generate_context!());
    if let Err(error) = result {
        eprintln!("Spiegelmond failed to start: {error}");
        std::process::exit(1);
    }
}
