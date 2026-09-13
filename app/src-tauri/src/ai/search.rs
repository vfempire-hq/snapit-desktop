// ai/search.rs — text search across the catalog for R·01.
//
// R·01 ships a plain-text search over filename + camera_make + camera_model
// + date parts. Semantic search over MobileCLIP embeddings lands in the
// R·01 M3-late slot once we finish the ONNX runtime plumbing.

use anyhow::Result;
use rusqlite::params;
use serde::Serialize;
use std::path::Path;

use crate::catalog;

#[derive(Serialize)]
pub struct SearchHit {
    pub id: String,
    pub path: String,
    pub taken_at: Option<String>,
    pub width: i64,
    pub height: i64,
    pub score: f32,
}

pub fn text_search(library_root: &Path, q: &str, limit: u32) -> Result<Vec<SearchHit>> {
    let conn = catalog::open(library_root)?;
    let q = q.trim().to_string();
    if q.is_empty() {
        return Ok(vec![]);
    }
    let like = format!("%{}%", q.to_lowercase());
    let mut stmt = conn.prepare(
        "SELECT id, rel_path, taken_at, COALESCE(width,0), COALESCE(height,0)
         FROM photos
         WHERE deleted_at IS NULL AND (
              LOWER(rel_path)    LIKE ?1
           OR LOWER(camera_make) LIKE ?1
           OR LOWER(camera_model) LIKE ?1
           OR LOWER(COALESCE(taken_at,'')) LIKE ?1
         )
         ORDER BY COALESCE(taken_at, imported_at) DESC
         LIMIT ?2",
    )?;
    let rows = stmt
        .query_map(params![like, limit], |r| {
            Ok(SearchHit {
                id: r.get(0)?,
                path: r.get(1)?,
                taken_at: r.get(2)?,
                width: r.get(3)?,
                height: r.get(4)?,
                score: 1.0,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}
