// SnapIT — Tauri backend entrypoint.
//
// R·01 M1: SQLite catalog + library scan.
// The catalog lives INSIDE the user's library folder as `_snapit/catalog.sqlite`.
// The library is portable — copy the folder to another machine and SnapIT there
// opens the same catalog. Your data outlives us.

use std::path::PathBuf;
use std::sync::Mutex;

use serde::Serialize;
use tauri::State;

mod catalog;
mod storage;
mod licence;

pub struct AppState {
    /// Currently-open library root, if any. Only one library open at a time.
    pub library: Mutex<Option<PathBuf>>,
    /// The last time we finished a scan on the open library.
    pub last_scan_at: Mutex<Option<chrono::DateTime<chrono::Utc>>>,
}

#[derive(Serialize)]
struct CatalogState {
    ready: bool,
    library_path: Option<String>,
    photo_count: u64,
    last_scan_at: Option<String>,
}

#[tauri::command]
async fn catalog_state(state: State<'_, AppState>) -> Result<CatalogState, String> {
    let library = state.library.lock().unwrap().clone();
    let last = state.last_scan_at.lock().unwrap().clone();
    let (photo_count, ready) = match &library {
        Some(root) => {
            let count = catalog::photo_count(root).unwrap_or(0);
            (count, true)
        }
        None => (0, true),
    };
    Ok(CatalogState {
        ready,
        library_path: library.map(|p| p.to_string_lossy().to_string()),
        photo_count,
        last_scan_at: last.map(|t| t.to_rfc3339()),
    })
}

#[tauri::command]
async fn catalog_open(path: String, state: State<'_, AppState>) -> Result<CatalogState, String> {
    let root = PathBuf::from(&path);
    catalog::open_or_init(&root).map_err(|e| e.to_string())?;
    *state.library.lock().unwrap() = Some(root.clone());
    let photo_count = catalog::photo_count(&root).unwrap_or(0);
    let last = state.last_scan_at.lock().unwrap().clone();
    Ok(CatalogState {
        ready: true,
        library_path: Some(root.to_string_lossy().to_string()),
        photo_count,
        last_scan_at: last.map(|t| t.to_rfc3339()),
    })
}

#[tauri::command]
async fn library_scan(path: String, state: State<'_, AppState>) -> Result<u64, String> {
    let root = PathBuf::from(&path);
    let ingested = storage::local::scan(&root).map_err(|e| e.to_string())?;
    *state.last_scan_at.lock().unwrap() = Some(chrono::Utc::now());
    tracing::info!("library_scan finished: {} photos indexed under {}", ingested, path);
    Ok(ingested)
}

#[tauri::command]
async fn catalog_recent(
    limit: u32,
    state: State<'_, AppState>,
) -> Result<Vec<catalog::PhotoRow>, String> {
    let library = state.library.lock().unwrap().clone().ok_or("no library open")?;
    catalog::recent(&library, limit).map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt().with_env_filter("info").init();
    tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(AppState {
            library: Mutex::new(None),
            last_scan_at: Mutex::new(None),
        })
        .invoke_handler(tauri::generate_handler![
            catalog_state,
            catalog_open,
            library_scan,
            catalog_recent,
        ])
        .run(tauri::generate_context!())
        .expect("SnapIT failed to boot");
}
