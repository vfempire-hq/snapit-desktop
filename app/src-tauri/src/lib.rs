// SnapIT — Tauri backend entrypoint.
//
// R·01 M1 + M2:
//   catalog     SQLite catalog inside the library folder
//   storage     fs walker, notify watcher, thumbnail generator
//   edit        non-destructive edit-stack persistence
//   licence     offline Ed25519 verification (M5 stub)

use std::path::PathBuf;
use std::sync::Mutex;

use serde::Serialize;
use tauri::State;

mod catalog;
mod edit;
mod licence;
mod storage;

pub struct AppState {
    pub library: Mutex<Option<PathBuf>>,
    pub last_scan_at: Mutex<Option<chrono::DateTime<chrono::Utc>>>,
    /// Holds the running folder watcher while a library is open.
    pub watch: Mutex<Option<storage::watch::LibraryWatch>>,
}

#[derive(Serialize)]
struct CatalogState {
    ready: bool,
    library_path: Option<String>,
    photo_count: u64,
    last_scan_at: Option<String>,
}

fn build_state(state: &AppState) -> CatalogState {
    let library = state.library.lock().unwrap().clone();
    let last = state.last_scan_at.lock().unwrap().clone();
    let photo_count = library.as_ref().and_then(|r| catalog::photo_count(r).ok()).unwrap_or(0);
    CatalogState {
        ready: true,
        library_path: library.map(|p| p.to_string_lossy().to_string()),
        photo_count,
        last_scan_at: last.map(|t| t.to_rfc3339()),
    }
}

#[tauri::command]
async fn catalog_state(state: State<'_, AppState>) -> Result<CatalogState, String> {
    Ok(build_state(&state))
}

#[tauri::command]
async fn catalog_open(path: String, state: State<'_, AppState>) -> Result<CatalogState, String> {
    let root = PathBuf::from(&path);
    catalog::open_or_init(&root).map_err(|e| e.to_string())?;
    *state.library.lock().unwrap() = Some(root.clone());

    // (Re)start the folder watcher on the newly-opened library.
    match storage::watch::LibraryWatch::start(root.clone()) {
        Ok(w) => *state.watch.lock().unwrap() = Some(w),
        Err(e) => tracing::warn!("could not start folder watch: {}", e),
    }

    Ok(build_state(&state))
}

#[tauri::command]
async fn library_scan(path: String, state: State<'_, AppState>) -> Result<u64, String> {
    let root = PathBuf::from(&path);
    let ingested = storage::local::scan(&root).map_err(|e| e.to_string())?;
    *state.last_scan_at.lock().unwrap() = Some(chrono::Utc::now());
    tracing::info!("scan finished: {} photos indexed under {}", ingested, path);
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

// ---------- thumbnails ----------

#[tauri::command]
async fn thumb_ensure(photo_id: String, state: State<'_, AppState>) -> Result<String, String> {
    let library = state.library.lock().unwrap().clone().ok_or("no library open")?;
    let (rel_path, content_hash) = catalog::path_and_hash_by_id(&library, &photo_id)
        .map_err(|e| e.to_string())?;
    let abs = library.join(&rel_path);
    storage::thumbs::ensure_thumbs(&library, &abs, &content_hash).map_err(|e| e.to_string())?;
    let path = storage::thumbs::thumb_asset_path(&library, &content_hash, 256);
    Ok(path.to_string_lossy().to_string())
}

// ---------- edits ----------

#[tauri::command]
async fn edit_get(photo_id: String, state: State<'_, AppState>) -> Result<Vec<edit::EditOp>, String> {
    let library = state.library.lock().unwrap().clone().ok_or("no library open")?;
    edit::get_stack(&library, &photo_id).map_err(|e| e.to_string())
}

#[tauri::command]
async fn edit_set(
    photo_id: String,
    stack: Vec<edit::EditOp>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let library = state.library.lock().unwrap().clone().ok_or("no library open")?;
    edit::set_stack(&library, &photo_id, &stack).map_err(|e| e.to_string())
}

#[tauri::command]
async fn edit_clear(photo_id: String, state: State<'_, AppState>) -> Result<(), String> {
    let library = state.library.lock().unwrap().clone().ok_or("no library open")?;
    edit::clear_stack(&library, &photo_id).map_err(|e| e.to_string())
}

// ---------- licence ----------

#[tauri::command]
async fn licence_status() -> Result<licence::LicenceStatus, String> {
    Ok(licence::compute_status())
}

#[tauri::command]
async fn licence_import(
    licence_json: String,
    signature_b64: String,
) -> Result<licence::LicenceStatus, String> {
    licence::import(&licence_json, &signature_b64).map_err(|e| e.to_string())?;
    Ok(licence::compute_status())
}

#[tauri::command]
async fn licence_forget() -> Result<licence::LicenceStatus, String> {
    licence::forget().map_err(|e| e.to_string())?;
    Ok(licence::compute_status())
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
            watch: Mutex::new(None),
        })
        .setup(|_app| {
            // Placeholder for future startup work (rehydrate last library, etc.).
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            catalog_state,
            catalog_open,
            library_scan,
            catalog_recent,
            thumb_ensure,
            edit_get,
            edit_set,
            edit_clear,
            licence_status,
            licence_import,
            licence_forget,
        ])
        .run(tauri::generate_context!())
        .expect("SnapIT failed to boot");
}
