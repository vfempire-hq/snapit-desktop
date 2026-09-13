// profile/mod.rs — SnapIT profiles (Family Pack).
//
// Design decisions:
//
// * Storage lives at <os-config>/SnapIT/profiles.json — same lane as prefs.
//   Profiles are PER-INSTALL, not per-library. Every library the user opens
//   is filtered/routed through the currently active profile.
//
// * PINs are hashed with Argon2id (params m=19456, t=2, p=1 — OWASP 2023).
//   The hash string carries its own salt + params so we can rotate any of
//   them without breaking existing PINs. PINs are 4-8 digits — kids do NOT
//   need to remember long strings; the attack surface is a stolen laptop,
//   not an online adversary.
//
// * The owner profile is `delete_forbidden` and can only be renamed, not
//   removed. That keeps the "who owns this install + Stripe licence" clear.
//
// * File is written atomically (tmp + rename) so a crash mid-save doesn't
//   corrupt the profile list.

use anyhow::{anyhow, Context, Result};
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::{Algorithm, Argon2, Params, Version};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

pub const FAMILY_PACK_MAX: usize = 5;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub id: String,
    pub name: String,
    #[serde(default = "default_role")]
    pub role: String, // "owner" | "family" | "kid"
    #[serde(default = "default_initials")]
    pub initials: String,
    /// gradient-v | gradient-e | gradient-l | gradient-k or free-form hex
    #[serde(default = "default_gradient")]
    pub gradient: String,
    #[serde(default)]
    pub kids: bool,
    /// pin_hash carries the Argon2id encoded string. None = no PIN set.
    #[serde(default)]
    pub pin_hash: Option<String>,
    #[serde(default = "default_autoplay")]
    pub autoplay_slides: bool,
    #[serde(default = "default_autoplay")]
    pub autoplay_previews: bool,
    /// User consent to build face embeddings from photos this profile owns.
    /// OFF by default — face grouping is opt-in per person, not per install.
    #[serde(default)]
    pub face_group_consent: bool,
    /// "this-profile" | "shared" | "ask"
    #[serde(default = "default_save_dest")]
    pub default_save: String,
    /// "none" | "13+" | "kid-safe"
    #[serde(default = "default_restrictions")]
    pub content_restrictions: String,
    #[serde(default = "default_language")]
    pub language: String,
    /// Show the platform UI overlay (TikTok/IG/YT chrome) on saved social
    /// content when it was captured as a screenshot. Clean extension/takeout
    /// imports never show chrome regardless. Default ON — most users
    /// screenshot more than they extension-save.
    #[serde(default = "default_true")]
    pub show_platform_chrome: bool,
    #[serde(default)]
    pub delete_forbidden: bool,
    #[serde(default)]
    pub created_at: String,
}

fn default_true() -> bool { true }

fn default_role() -> String { "family".into() }
fn default_initials() -> String { "?".into() }
fn default_gradient() -> String { "gradient-v".into() }
fn default_autoplay() -> bool { true }
fn default_save_dest() -> String { "this-profile".into() }
fn default_restrictions() -> String { "none".into() }
fn default_language() -> String { "en-GB".into() }

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProfileStore {
    #[serde(default)]
    pub profiles: Vec<Profile>,
    #[serde(default)]
    pub active_profile_id: Option<String>,
}

fn store_path() -> Option<PathBuf> {
    Some(dirs::config_dir()?.join("SnapIT").join("profiles.json"))
}

pub fn load() -> ProfileStore {
    let Some(path) = store_path() else {
        return ProfileStore::default();
    };
    match fs::read_to_string(&path) {
        Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
        Err(_) => ProfileStore::default(),
    }
}

pub fn save(store: &ProfileStore) -> Result<()> {
    let path = store_path().context("no config dir")?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).context("mkdir config dir")?;
    }
    let json = serde_json::to_string_pretty(store)?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, json)?;
    fs::rename(tmp, path)?;
    Ok(())
}

/// Ensure at least an Owner profile exists. Called once at app boot so a
/// brand-new install has something to pick.
pub fn bootstrap_if_empty() -> Result<()> {
    let mut store = load();
    if store.profiles.is_empty() {
        let owner = Profile {
            id: uuid::Uuid::now_v7().to_string(),
            name: "Owner".to_string(),
            role: "owner".to_string(),
            initials: "O".to_string(),
            gradient: "gradient-v".to_string(),
            kids: false,
            pin_hash: None,
            autoplay_slides: true,
            autoplay_previews: true,
            face_group_consent: false,
            default_save: "this-profile".to_string(),
            content_restrictions: "none".to_string(),
            language: default_language(),
            show_platform_chrome: true,
            delete_forbidden: true,
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        store.active_profile_id = Some(owner.id.clone());
        store.profiles.push(owner);
        save(&store)?;
    }
    Ok(())
}

/// Create a new profile. Validates seat cap + PIN if provided.
pub fn create(new: Profile, pin: Option<String>) -> Result<Profile> {
    let mut store = load();
    if store.profiles.len() >= FAMILY_PACK_MAX {
        return Err(anyhow!("family_pack_full"));
    }
    let mut p = new;
    if p.id.is_empty() {
        p.id = uuid::Uuid::now_v7().to_string();
    }
    if p.created_at.is_empty() {
        p.created_at = chrono::Utc::now().to_rfc3339();
    }
    if let Some(pin_val) = pin {
        p.pin_hash = Some(hash_pin(&pin_val)?);
    }
    // First-created profile becomes the owner automatically.
    if store.profiles.is_empty() {
        p.role = "owner".into();
        p.delete_forbidden = true;
        store.active_profile_id = Some(p.id.clone());
    }
    let out = p.clone();
    store.profiles.push(p);
    save(&store)?;
    Ok(out)
}

pub fn update(profile_id: &str, patch: ProfilePatch) -> Result<Profile> {
    let mut store = load();
    let p = store
        .profiles
        .iter_mut()
        .find(|p| p.id == profile_id)
        .ok_or_else(|| anyhow!("profile_not_found"))?;

    macro_rules! apply {
        ($field:ident) => {
            if let Some(v) = patch.$field {
                p.$field = v;
            }
        };
    }
    apply!(name);
    apply!(initials);
    apply!(gradient);
    apply!(kids);
    apply!(autoplay_slides);
    apply!(autoplay_previews);
    apply!(face_group_consent);
    apply!(default_save);
    apply!(content_restrictions);
    apply!(language);
    apply!(show_platform_chrome);
    let out = p.clone();
    save(&store)?;
    Ok(out)
}

pub fn delete(profile_id: &str) -> Result<()> {
    let mut store = load();
    let idx = store
        .profiles
        .iter()
        .position(|p| p.id == profile_id)
        .ok_or_else(|| anyhow!("profile_not_found"))?;
    if store.profiles[idx].delete_forbidden {
        return Err(anyhow!("owner_locked"));
    }
    store.profiles.remove(idx);
    if store.active_profile_id.as_deref() == Some(profile_id) {
        store.active_profile_id = store.profiles.first().map(|p| p.id.clone());
    }
    save(&store)?;
    Ok(())
}

/// Set / change / clear a profile PIN. Passing None clears the PIN.
pub fn set_pin(profile_id: &str, new_pin: Option<String>) -> Result<()> {
    if let Some(ref pin) = new_pin {
        validate_pin_shape(pin)?;
    }
    let mut store = load();
    let p = store
        .profiles
        .iter_mut()
        .find(|p| p.id == profile_id)
        .ok_or_else(|| anyhow!("profile_not_found"))?;
    p.pin_hash = match new_pin {
        Some(pin) => Some(hash_pin(&pin)?),
        None => None,
    };
    save(&store)
}

/// Constant-time PIN verify (verify_password inside argon2 crate is c-t).
/// Returns Ok(true) on match, Ok(false) on mismatch. Never leaks timing info.
pub fn verify_pin(profile_id: &str, pin: &str) -> Result<bool> {
    let store = load();
    let p = store
        .profiles
        .iter()
        .find(|p| p.id == profile_id)
        .ok_or_else(|| anyhow!("profile_not_found"))?;
    let Some(ref hash_str) = p.pin_hash else {
        return Ok(true); // no PIN set → always unlocks
    };
    let parsed = PasswordHash::new(hash_str)
        .map_err(|e| anyhow!("pin_hash_parse: {e}"))?;
    Ok(argon2_hasher()
        .verify_password(pin.as_bytes(), &parsed)
        .is_ok())
}

pub fn set_active(profile_id: &str) -> Result<()> {
    let mut store = load();
    if !store.profiles.iter().any(|p| p.id == profile_id) {
        return Err(anyhow!("profile_not_found"));
    }
    store.active_profile_id = Some(profile_id.to_string());
    save(&store)
}

/// Small patch struct so update commands can carry only the changed fields.
#[derive(Debug, Default, Clone, Deserialize)]
pub struct ProfilePatch {
    pub name: Option<String>,
    pub initials: Option<String>,
    pub gradient: Option<String>,
    pub kids: Option<bool>,
    pub autoplay_slides: Option<bool>,
    pub autoplay_previews: Option<bool>,
    pub face_group_consent: Option<bool>,
    pub default_save: Option<String>,
    pub content_restrictions: Option<String>,
    pub language: Option<String>,
    pub show_platform_chrome: Option<bool>,
}

/// Public snapshot the frontend receives — hides the PIN hash by omission.
#[derive(Debug, Clone, Serialize)]
pub struct ProfileView {
    pub id: String,
    pub name: String,
    pub role: String,
    pub initials: String,
    pub gradient: String,
    pub kids: bool,
    pub locked: bool, // = pin_hash.is_some()
    pub autoplay_slides: bool,
    pub autoplay_previews: bool,
    pub face_group_consent: bool,
    pub default_save: String,
    pub content_restrictions: String,
    pub language: String,
    pub delete_forbidden: bool,
    pub created_at: String,
}

impl From<&Profile> for ProfileView {
    fn from(p: &Profile) -> Self {
        ProfileView {
            id: p.id.clone(),
            name: p.name.clone(),
            role: p.role.clone(),
            initials: p.initials.clone(),
            gradient: p.gradient.clone(),
            kids: p.kids,
            locked: p.pin_hash.is_some(),
            autoplay_slides: p.autoplay_slides,
            autoplay_previews: p.autoplay_previews,
            face_group_consent: p.face_group_consent,
            default_save: p.default_save.clone(),
            content_restrictions: p.content_restrictions.clone(),
            language: p.language.clone(),
            delete_forbidden: p.delete_forbidden,
            created_at: p.created_at.clone(),
        }
    }
}

fn hash_pin(pin: &str) -> Result<String> {
    validate_pin_shape(pin)?;
    let salt = SaltString::generate(&mut OsRng);
    let hash = argon2_hasher()
        .hash_password(pin.as_bytes(), &salt)
        .map_err(|e| anyhow!("argon2_hash: {e}"))?
        .to_string();
    Ok(hash)
}

fn validate_pin_shape(pin: &str) -> Result<()> {
    let n = pin.chars().count();
    if !(4..=8).contains(&n) {
        return Err(anyhow!("pin_must_be_4_to_8_digits"));
    }
    if !pin.chars().all(|c| c.is_ascii_digit()) {
        return Err(anyhow!("pin_must_be_digits"));
    }
    Ok(())
}

fn argon2_hasher() -> Argon2<'static> {
    // OWASP 2023 baseline for interactive login: 19 MB memory, 2 iterations, 1 lane.
    let params = Params::new(19_456, 2, 1, None).expect("argon2 params");
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
}

// ============================================================
//  TESTS
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pin_hash_verify_roundtrip() {
        let hash = hash_pin("4711").expect("hash");
        assert!(PasswordHash::new(&hash).is_ok());
        // Round-trip verification via a fake profile
        let p = Profile {
            id: "x".into(),
            name: "T".into(),
            role: "family".into(),
            initials: "T".into(),
            gradient: "gradient-v".into(),
            kids: false,
            pin_hash: Some(hash),
            autoplay_slides: true,
            autoplay_previews: true,
            face_group_consent: false,
            default_save: "this-profile".into(),
            content_restrictions: "none".into(),
            language: "en-GB".into(),
            show_platform_chrome: true,
            delete_forbidden: false,
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        let parsed = PasswordHash::new(p.pin_hash.as_ref().unwrap()).unwrap();
        assert!(argon2_hasher().verify_password(b"4711", &parsed).is_ok());
        assert!(argon2_hasher().verify_password(b"9999", &parsed).is_err());
    }

    #[test]
    fn pin_shape_rules() {
        assert!(validate_pin_shape("1234").is_ok());
        assert!(validate_pin_shape("12345678").is_ok());
        assert!(validate_pin_shape("123").is_err(), "too short");
        assert!(validate_pin_shape("123456789").is_err(), "too long");
        assert!(validate_pin_shape("12a4").is_err(), "non-digit");
        assert!(validate_pin_shape("").is_err(), "empty");
    }
}
