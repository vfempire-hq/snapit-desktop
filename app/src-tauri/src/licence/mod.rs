// licence/mod.rs — offline Ed25519 licence verification + local persistence.
//
// R·01 M5 shipped:
//   - Public key of the licence-issuing Ed25519 keypair is baked in below.
//     The matching private key lives ONLY on the purchase-backend
//     Cloudflare Worker (Wrangler secret `LICENCE_SIGNING_KEY`).
//   - Users receive `snapit.licence.json` + `snapit.licence.sig` by email
//     after purchase. They import the licence via the app; we verify the
//     Ed25519 signature offline and stash the pair under
//     `<app-config>/licence.json` + `.sig`. Nothing goes over the wire.
//   - `licence_status()` is safe to call before a licence is imported;
//     it returns { activated: false } instead of an error.
//
// Permanence Guarantee (house LAW 2026-09-13): if VF Empire discontinues
// SnapIT, this file — the licence format, signing scheme, and public key —
// gets published in a public spec. Anyone can then continue verifying
// existing licences forever.

use anyhow::{anyhow, Context, Result};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const LICENCE_PUB_KEY_HEX: &str =
    "b9f9abbd93a38424712cabaad2b684017aa3bb6e0b80cef4e12a70f9787d66c1";

#[derive(Serialize, Deserialize, Clone)]
pub struct Licence {
    pub email: String,
    pub tier: String, // "personal" | "family" | "pro"
    pub issued_at: i64,
    pub product: String, // "snapit"
    pub major_version: u32,
}

#[derive(Serialize)]
pub struct LicenceStatus {
    pub activated: bool,
    pub email: Option<String>,
    pub tier: Option<String>,
    pub issued_at: Option<i64>,
    pub major_version: Option<u32>,
    pub trial_days_remaining: Option<u32>,
}

pub fn verify(licence_json: &str, signature_b64: &str) -> Result<Licence> {
    let pk_bytes = hex::decode(LICENCE_PUB_KEY_HEX).context("decode LICENCE_PUB_KEY_HEX")?;
    let pk_arr: [u8; 32] = pk_bytes
        .try_into()
        .map_err(|_| anyhow!("licence pubkey wrong length"))?;
    let pk = VerifyingKey::from_bytes(&pk_arr)?;

    let sig_bytes = B64.decode(signature_b64).context("decode signature_b64")?;
    let sig = Signature::try_from(sig_bytes.as_slice())
        .map_err(|_| anyhow!("signature wrong length"))?;
    pk.verify(licence_json.as_bytes(), &sig)
        .map_err(|_| anyhow!("licence signature invalid"))?;

    let l: Licence = serde_json::from_str(licence_json).context("parse licence json")?;
    if l.product != "snapit" {
        return Err(anyhow!("licence is for another product: {}", l.product));
    }
    if l.major_version != 1 {
        return Err(anyhow!(
            "licence is for major version {} but this app is v1",
            l.major_version
        ));
    }
    Ok(l)
}

fn licence_dir() -> Result<PathBuf> {
    let base = dirs::config_dir().ok_or_else(|| anyhow!("no config dir on this OS"))?;
    let dir = base.join("SnapIT");
    std::fs::create_dir_all(&dir).with_context(|| format!("create {:?}", dir))?;
    Ok(dir)
}

fn licence_path() -> Result<PathBuf> {
    Ok(licence_dir()?.join("licence.json"))
}

fn signature_path() -> Result<PathBuf> {
    Ok(licence_dir()?.join("licence.sig"))
}

pub fn import(licence_json: &str, signature_b64: &str) -> Result<Licence> {
    let l = verify(licence_json, signature_b64)?;
    std::fs::write(licence_path()?, licence_json)?;
    std::fs::write(signature_path()?, signature_b64)?;
    Ok(l)
}

pub fn read_installed() -> Option<Licence> {
    let l_path = licence_path().ok()?;
    let s_path = signature_path().ok()?;
    let json = std::fs::read_to_string(&l_path).ok()?;
    let sig = std::fs::read_to_string(&s_path).ok()?;
    verify(&json, &sig).ok()
}

pub fn forget() -> Result<()> {
    let _ = std::fs::remove_file(licence_path()?);
    let _ = std::fs::remove_file(signature_path()?);
    Ok(())
}

/// 14-day free trial from first launch. Trial state lives in
/// `<config>/trial.txt` — a unix-epoch of first_launch. We never enforce
/// the trial hard (SnapIT is a photo-library manager, users' data stays
/// visible forever) — but we nudge them to buy after 14 days.
pub fn trial_days_remaining() -> Option<u32> {
    let path = licence_dir().ok()?.join("trial.txt");
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs() as i64;
    let start = match std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| s.trim().parse::<i64>().ok())
    {
        Some(v) => v,
        None => {
            let _ = std::fs::write(&path, now.to_string());
            now
        }
    };
    let elapsed_days = ((now - start) / 86_400) as i32;
    let remaining = 14 - elapsed_days;
    Some(remaining.max(0) as u32)
}

pub fn compute_status() -> LicenceStatus {
    match read_installed() {
        Some(l) => LicenceStatus {
            activated: true,
            email: Some(l.email),
            tier: Some(l.tier),
            issued_at: Some(l.issued_at),
            major_version: Some(l.major_version),
            trial_days_remaining: None,
        },
        None => LicenceStatus {
            activated: false,
            email: None,
            tier: None,
            issued_at: None,
            major_version: None,
            trial_days_remaining: trial_days_remaining(),
        },
    }
}

/// A helper an anti-tamper future upgrade can use — right now it just wraps `Path`.
#[allow(dead_code)]
pub fn path_hint(_root: &Path) -> &'static str {
    "SnapIT stores your licence pair under the OS config dir."
}
