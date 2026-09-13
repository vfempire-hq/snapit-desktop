// storage/watch.rs — filesystem watcher for the open library.
//
// Uses `notify` to get raw fs events, debounces them by 750 ms so a burst of
// camera-card-copy events collapses to a single rescan of the affected
// subtrees. Ignored: our own _snapit/ dir and anything that matches the
// scanner's existing skip list.

use anyhow::Result;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::storage::local;

/// Owns the running watcher. Drop this to stop watching.
pub struct LibraryWatch {
    _watcher: RecommendedWatcher,
}

impl LibraryWatch {
    pub fn start(library_root: PathBuf) -> Result<Self> {
        let (tx, rx) = channel::<notify::Event>();
        let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
            if let Ok(ev) = res {
                let _ = tx.send(ev);
            }
        })?;
        watcher.watch(&library_root, RecursiveMode::Recursive)?;

        let root_for_thread = library_root.clone();
        let last_event: Arc<Mutex<Option<Instant>>> = Arc::new(Mutex::new(None));
        let last_event_thr = last_event.clone();

        std::thread::spawn(move || {
            let mut pending: Vec<PathBuf> = Vec::new();
            let debounce = Duration::from_millis(750);
            loop {
                match rx.recv_timeout(debounce) {
                    Ok(ev) => {
                        for p in ev.paths {
                            if !is_ignored(&p) {
                                pending.push(p);
                            }
                        }
                        *last_event_thr.lock().unwrap() = Some(Instant::now());
                    }
                    Err(_) => {
                        // Idle: fire rescan if we have pending events and debounce has elapsed.
                        let ready = last_event_thr
                            .lock()
                            .unwrap()
                            .map(|t| t.elapsed() >= debounce)
                            .unwrap_or(false);
                        if !pending.is_empty() && ready {
                            tracing::info!("watch: rescanning after {} pending events", pending.len());
                            let _ = local::scan(&root_for_thread);
                            pending.clear();
                            *last_event_thr.lock().unwrap() = None;
                        }
                    }
                }
            }
        });

        Ok(LibraryWatch { _watcher: watcher })
    }
}

fn is_ignored(path: &Path) -> bool {
    for c in path.components() {
        if let std::path::Component::Normal(os) = c {
            let s = os.to_string_lossy();
            if s == crate::catalog::CATALOG_DIR
                || s.starts_with('.')
                || s == "@eaDir"
                || s == "$RECYCLE.BIN"
                || s == "System Volume Information"
            {
                return true;
            }
        }
    }
    false
}
