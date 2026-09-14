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
use tauri::{Emitter, Manager, State};
use tauri_plugin_updater::UpdaterExt;

pub mod ai;
pub mod catalog;
pub mod edit;
pub mod events;
pub mod licence;
pub mod prefs;
pub mod profile;
pub mod storage;

pub struct AppState {
    pub library: Mutex<Option<PathBuf>>,
    pub last_scan_at: Mutex<Option<chrono::DateTime<chrono::Utc>>>,
    /// Holds the running folder watcher while a library is open.
    pub watch: Mutex<Option<storage::watch::LibraryWatch>>,
    /// Update returned by the launch-time updater check, waiting for the
    /// user's Install click.
    pub pending_update: Mutex<Option<tauri_plugin_updater::Update>>,
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
    prefs::note_library(&root);

    // (Re)start the folder watcher on the newly-opened library.
    match storage::watch::LibraryWatch::start(root.clone()) {
        Ok(w) => *state.watch.lock().unwrap() = Some(w),
        Err(e) => tracing::warn!("could not start folder watch: {}", e),
    }

    Ok(build_state(&state))
}

#[tauri::command]
async fn prefs_get() -> Result<prefs::Prefs, String> {
    Ok(prefs::load())
}

#[tauri::command]
async fn library_scan(
    path: String,
    state: State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<u64, String> {
    use std::sync::Arc;
    let root = PathBuf::from(&path);
    let app_for_cb = app.clone();
    let cb: storage::local::ScanProgress = Arc::new(move |ev| {
        // Best-effort emit — if the window is gone, ignore.
        let _ = app_for_cb.emit("snapit://scan/progress", ev);
    });
    let ingested =
        storage::local::scan_with_progress(&root, Some(cb)).map_err(|e| e.to_string())?;
    *state.last_scan_at.lock().unwrap() = Some(chrono::Utc::now());
    tracing::info!("scan finished: {} media indexed under {}", ingested, path);

    // Kick off event re-clustering after every scan. Non-blocking failure —
    // clustering is best-effort, we always fall back to the flat grid.
    match events::recluster(&root) {
        Ok(n) => tracing::info!("event clustering: {} events created", n),
        Err(e) => tracing::warn!("event clustering failed: {}", e),
    }
    Ok(ingested)
}

#[tauri::command]
async fn events_recluster(state: State<'_, AppState>) -> Result<usize, String> {
    let library = state.library.lock().unwrap().clone().ok_or("no library open")?;
    events::recluster(&library).map_err(|e| e.to_string())
}

#[tauri::command]
async fn events_list(
    limit: u32,
    state: State<'_, AppState>,
) -> Result<Vec<catalog::EventRow>, String> {
    let library = state.library.lock().unwrap().clone().ok_or("no library open")?;
    catalog::list_events(&library, limit).map_err(|e| e.to_string())
}

#[tauri::command]
async fn catalog_recent(
    limit: u32,
    min_rating: Option<i32>,
    state: State<'_, AppState>,
) -> Result<Vec<catalog::PhotoRow>, String> {
    let library = state.library.lock().unwrap().clone().ok_or("no library open")?;
    catalog::recent_filtered(&library, limit, min_rating.unwrap_or(0)).map_err(|e| e.to_string())
}

/// Frontend-friendly row: everything the mock tile needs to render.
/// `thumb_path` is a raw filesystem path — the frontend runs it through
/// convertFileSrc() to turn it into an asset:// URL. Not converting here
/// keeps the Rust side platform-agnostic (Windows uses http://asset.localhost/,
/// Mac/Linux use asset://localhost/).
#[derive(serde::Serialize)]
struct HydratedRow {
    id: String,
    path: String,
    taken_at: Option<String>,
    width: i64,
    height: i64,
    xmp_rating: Option<i32>,
    kind: String,
    camera: Option<String>,
    caption: Option<String>,
    duration_s: Option<i64>,
    content_hash: String,
    /// filesystem path — pass through convertFileSrc() before assigning to <img>
    thumb_path: String,
}

/// Recent photos WITH precomputed thumbnail paths. Cheap because thumb_asset_path
/// is deterministic: it just concatenates library_root + hash-shard + hash + size.
/// The thumbnail file might not exist yet on disk — the frontend can fall back to
/// a placeholder + call thumb_ensure lazily as the user scrolls.
#[tauri::command]
async fn catalog_recent_hydrated(
    limit: u32,
    min_rating: Option<i32>,
    state: State<'_, AppState>,
) -> Result<Vec<HydratedRow>, String> {
    let library = state.library.lock().unwrap().clone().ok_or("no library open")?;
    let rows = catalog::recent_filtered(&library, limit, min_rating.unwrap_or(0))
        .map_err(|e| e.to_string())?;
    let hydrated = rows
        .into_iter()
        .map(|r| {
            let thumb_path = storage::thumbs::thumb_asset_path(&library, &r.content_hash, 256)
                .to_string_lossy()
                .to_string();
            HydratedRow {
                id: r.id,
                path: r.path,
                taken_at: r.taken_at,
                width: r.width,
                height: r.height,
                xmp_rating: r.xmp_rating,
                kind: r.kind,
                camera: r.camera,
                caption: r.caption,
                duration_s: r.duration_s,
                content_hash: r.content_hash,
                thumb_path,
            }
        })
        .collect();
    Ok(hydrated)
}

// ---------- duplicates ----------

#[tauri::command]
async fn catalog_duplicates(
    limit: u32,
    state: State<'_, AppState>,
) -> Result<Vec<catalog::DuplicateGroup>, String> {
    let library = state.library.lock().unwrap().clone().ok_or("no library open")?;
    catalog::duplicate_groups(&library, limit).map_err(|e| e.to_string())
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

// ---------- reveal in native file manager ----------

#[tauri::command]
async fn reveal_in_folder(
    photo_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let library = state.library.lock().unwrap().clone().ok_or("no library open")?;
    let (rel, _) = catalog::path_and_hash_by_id(&library, &photo_id).map_err(|e| e.to_string())?;
    let abs = library.join(rel);

    #[cfg(target_os = "macos")]
    let cmd = std::process::Command::new("open")
        .arg("-R")
        .arg(abs.as_os_str())
        .spawn();
    #[cfg(target_os = "windows")]
    let cmd = std::process::Command::new("explorer")
        .arg(format!("/select,{}", abs.to_string_lossy()))
        .spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let cmd = std::process::Command::new("xdg-open")
        .arg(abs.parent().unwrap_or(&abs))
        .spawn();

    cmd.map_err(|e| e.to_string())?;
    Ok(())
}

// ---------- rating ----------

#[tauri::command]
async fn rating_set(
    photo_id: String,
    rating: i32,
    write_xmp: bool,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let library = state.library.lock().unwrap().clone().ok_or("no library open")?;
    let clamped = rating.clamp(0, 5);

    // Update SQLite first — always cheap, always safe.
    let conn = catalog::open(&library).map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE photos SET xmp_rating = ?1 WHERE id = ?2",
        rusqlite::params![clamped, &photo_id],
    )
    .map_err(|e| e.to_string())?;

    // Optionally write back to the XMP sidecar so Lightroom / Bridge see it.
    if write_xmp {
        let (rel, _) = catalog::path_and_hash_by_id(&library, &photo_id)
            .map_err(|e| e.to_string())?;
        let abs = library.join(&rel);
        storage::xmp::write_rating(&abs, clamped).map_err(|e| e.to_string())?;
    }
    Ok(())
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

// ---------- export ----------

#[tauri::command]
async fn edit_export(
    request: edit::export::ExportRequest,
    state: State<'_, AppState>,
) -> Result<edit::export::ExportReport, String> {
    let library = state.library.lock().unwrap().clone().ok_or("no library open")?;
    edit::export::export_one(&library, request).map_err(|e| e.to_string())
}

// ---------- search ----------

#[tauri::command]
async fn search_text(
    q: String,
    limit: u32,
    state: State<'_, AppState>,
) -> Result<Vec<ai::search::SearchHit>, String> {
    let library = state.library.lock().unwrap().clone().ok_or("no library open")?;
    ai::search::text_search(&library, &q, limit).map_err(|e| e.to_string())
}

// ---------- profiles ----------
// Family Pack profile management. Backend (Argon2id PIN, atomic writes) lives
// in profile/mod.rs; these are the thin Tauri shells the mock UI calls.

#[tauri::command]
async fn profile_list() -> Result<Vec<profile::ProfileView>, String> {
    Ok(profile::list_views())
}

#[tauri::command]
async fn profile_active() -> Result<Option<profile::ProfileView>, String> {
    Ok(profile::active_view())
}

#[tauri::command]
async fn profile_bootstrap() -> Result<(), String> {
    profile::bootstrap_if_empty().map_err(|e| e.to_string())
}

#[tauri::command]
async fn profile_create(
    input: profile::ProfileInput,
    pin: Option<String>,
) -> Result<profile::ProfileView, String> {
    profile::create_from_input(input, pin).map_err(|e| e.to_string())
}

#[tauri::command]
async fn profile_update(
    id: String,
    patch: profile::ProfilePatch,
) -> Result<profile::ProfileView, String> {
    let p = profile::update(&id, patch).map_err(|e| e.to_string())?;
    Ok(profile::ProfileView::from(&p))
}

#[tauri::command]
async fn profile_delete(id: String) -> Result<(), String> {
    profile::delete(&id).map_err(|e| e.to_string())
}

#[tauri::command]
async fn profile_set_pin(id: String, pin: Option<String>) -> Result<(), String> {
    profile::set_pin(&id, pin).map_err(|e| e.to_string())
}

#[tauri::command]
async fn profile_verify_pin(id: String, pin: String) -> Result<bool, String> {
    profile::verify_pin(&id, &pin).map_err(|e| e.to_string())
}

#[tauri::command]
async fn profile_set_active(id: String) -> Result<(), String> {
    profile::set_active(&id).map_err(|e| e.to_string())
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

// ---------- updater trigger from the frontend ----------

#[tauri::command]
async fn update_install(
    state: State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let update = state
        .pending_update
        .lock()
        .unwrap()
        .take()
        .ok_or("no update pending")?;

    let app_for_progress = app.clone();
    update
        .download_and_install(
            move |chunk, total| {
                // Emit progress so the frontend can show a bar.
                let _ = app_for_progress.emit(
                    "snapit://update/progress",
                    serde_json::json!({
                        "chunk": chunk,
                        "total": total,
                    }),
                );
            },
            || {
                // Download finished; install about to run.
            },
        )
        .await
        .map_err(|e| e.to_string())?;

    // On success the updater plugin will relaunch the app itself.
    Ok(())
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
            pending_update: Mutex::new(None),
        })
        .setup(|app| {
            // Make sure at least one profile exists (Owner) so the picker
            // has something to render on a fresh install.
            if let Err(e) = profile::bootstrap_if_empty() {
                tracing::warn!("profile bootstrap failed: {}", e);
            }

            // Auto-reopen the last library if it's still on disk.
            let p = prefs::load();
            if let Some(last) = p.last_library {
                let root = PathBuf::from(&last);
                if root.is_dir() {
                    if catalog::open_or_init(&root).is_ok() {
                        let state = app.state::<AppState>();
                        *state.library.lock().unwrap() = Some(root.clone());
                        match storage::watch::LibraryWatch::start(root) {
                            Ok(w) => *state.watch.lock().unwrap() = Some(w),
                            Err(e) => tracing::warn!("watch start after rehydrate: {}", e),
                        }
                    }
                } else {
                    tracing::info!("last library {} no longer exists on disk", last);
                }
            }

            // Kick off a background update check ~3s after launch. If a signed
            // update is found on our /updates endpoint the frontend gets a
            // 'snapit://update/available' event with { version, notes } and can
            // present its own toast; user clicks 'Install' → runs the
            // update_install command below.
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                match handle.updater() {
                    Ok(updater) => match updater.check().await {
                        Ok(Some(update)) => {
                            tracing::info!("update available -> {} (v0.1.7 test path)", update.version);
                            let _ = handle.emit(
                                "snapit://update/available",
                                serde_json::json!({
                                    "version": update.version,
                                    "notes":   update.body.clone().unwrap_or_default(),
                                }),
                            );
                            // Store the pending update on the AppState so
                            // update_install can pull it back out.
                            let state = handle.state::<AppState>();
                            *state.pending_update.lock().unwrap() = Some(update);
                        }
                        Ok(None) => tracing::info!("no update available"),
                        Err(e) => tracing::warn!("updater check failed: {}", e),
                    },
                    Err(e) => tracing::warn!("updater init failed: {}", e),
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            prefs_get,
            catalog_state,
            catalog_open,
            library_scan,
            catalog_recent,
            catalog_recent_hydrated,
            catalog_duplicates,
            events_recluster,
            events_list,
            thumb_ensure,
            reveal_in_folder,
            rating_set,
            edit_get,
            edit_set,
            edit_clear,
            edit_export,
            search_text,
            profile_list,
            profile_active,
            profile_bootstrap,
            profile_create,
            profile_update,
            profile_delete,
            profile_set_pin,
            profile_verify_pin,
            profile_set_active,
            licence_status,
            licence_import,
            licence_forget,
            update_install,
        ])
        .run(tauri::generate_context!())
        .expect("SnapIT failed to boot");
}
