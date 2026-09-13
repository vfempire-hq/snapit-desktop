// edit/mod.rs — non-destructive edit stack.
//
// Every photo has AT MOST ONE row in `edits` (SQLite). The row holds a JSON
// array of ordered edit ops. Ops are pure — the same photo + same stack ⇒ the
// same output pixel-for-pixel. R·01 ships the primitive edit set below. R·02
// adds RAW dev + upscale + denoise as new op names.

use anyhow::{Context, Result};
use chrono::Utc;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::catalog;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "op")]
#[serde(rename_all = "lowercase")]
pub enum EditOp {
    /// Non-destructive crop in relative coordinates [0..1].
    Crop { x: f32, y: f32, w: f32, h: f32 },
    /// Rotation in degrees clockwise. 0 / 90 / 180 / 270 supported in R·01.
    Rotate { deg: i32 },
    /// Exposure compensation in EV stops. Typical range [-3, +3].
    Expose { ev: f32 },
    /// Contrast adjustment. Typical range [-1, +1].
    Contrast { amount: f32 },
    /// Saturation adjustment. Typical range [-1, +1].
    Sat { amount: f32 },
    /// White-balance in kelvin + tint. Positive kelvin = warmer.
    #[serde(rename = "wb")]
    WhiteBalance { kelvin: f32, tint: f32 },
}

pub fn get_stack(library_root: &Path, photo_id: &str) -> Result<Vec<EditOp>> {
    let conn = catalog::open(library_root)?;
    let json: Option<String> = conn
        .query_row(
            "SELECT stack_json FROM edits WHERE photo_id = ?1",
            params![photo_id],
            |r| r.get(0),
        )
        .ok();
    match json {
        Some(j) => Ok(serde_json::from_str(&j).unwrap_or_default()),
        None => Ok(Vec::new()),
    }
}

pub fn set_stack(library_root: &Path, photo_id: &str, stack: &[EditOp]) -> Result<()> {
    let conn = catalog::open(library_root)?;
    let json = serde_json::to_string(stack).context("serialize edit stack")?;
    conn.execute(
        r#"
        INSERT INTO edits (photo_id, stack_json, updated_at) VALUES (?1, ?2, ?3)
        ON CONFLICT(photo_id) DO UPDATE SET
            stack_json = excluded.stack_json,
            updated_at = excluded.updated_at
        "#,
        params![photo_id, json, Utc::now().to_rfc3339()],
    )?;
    Ok(())
}

pub fn clear_stack(library_root: &Path, photo_id: &str) -> Result<()> {
    let conn = catalog::open(library_root)?;
    conn.execute("DELETE FROM edits WHERE photo_id = ?1", params![photo_id])?;
    Ok(())
}
