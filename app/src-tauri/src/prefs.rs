// prefs.rs — tiny persistent app preferences file.
//
// Stored under the OS-standard config dir (dirs::config_dir + "SnapIT/prefs.json")
// so it works alongside the licence file we keep in the same tree. Keep this
// intentionally minimal — every field is optional so old files stay readable.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Prefs {
    #[serde(default)]
    pub last_library: Option<String>,
    #[serde(default)]
    pub recent_libraries: Vec<String>,
}

fn prefs_path() -> Option<PathBuf> {
    let base = dirs::config_dir()?;
    Some(base.join("SnapIT").join("prefs.json"))
}

pub fn load() -> Prefs {
    let Some(path) = prefs_path() else {
        return Prefs::default();
    };
    match fs::read_to_string(&path) {
        Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
        Err(_) => Prefs::default(),
    }
}

pub fn save(prefs: &Prefs) -> Result<()> {
    let path = prefs_path().context("no config dir on this platform")?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).context("mkdir config dir")?;
    }
    let json = serde_json::to_string_pretty(prefs)?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, json)?;
    fs::rename(tmp, path)?;
    Ok(())
}

pub fn note_library(path: &Path) {
    let mut p = load();
    let s = path.to_string_lossy().to_string();
    p.last_library = Some(s.clone());
    p.recent_libraries.retain(|r| r != &s);
    p.recent_libraries.insert(0, s);
    p.recent_libraries.truncate(5);
    let _ = save(&p);
}
