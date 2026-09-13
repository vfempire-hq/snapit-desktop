// edit/export.rs — export a photo (edit stack baked in) to a destination file.
//
// Path safety: exports must land under a user-chosen directory. The Tauri
// dialog plugin surfaces the picker and returns an absolute path; we
// validate it exists and is writable before opening the file.

use anyhow::{anyhow, Context, Result};
use image::{codecs::jpeg::JpegEncoder, ColorType, ImageEncoder};
use rusqlite::params;
use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};

use crate::catalog;
use super::apply::apply_stack;
use super::get_stack;

#[derive(Debug, serde::Deserialize)]
pub struct ExportRequest {
    pub photo_id: String,
    pub destination: PathBuf, // absolute path to write to
    #[serde(default = "default_quality")]
    pub quality: u8, // 1..=100 JPEG quality
    #[serde(default)]
    pub max_edge: Option<u32>, // downscale so longer edge ≤ this
}

fn default_quality() -> u8 {
    92
}

#[derive(Debug, serde::Serialize)]
pub struct ExportReport {
    pub written_path: String,
    pub bytes: u64,
    pub width: u32,
    pub height: u32,
    pub applied_ops: usize,
}

pub fn export_one(library_root: &Path, req: ExportRequest) -> Result<ExportReport> {
    if !req.destination.is_absolute() {
        return Err(anyhow!("destination path must be absolute"));
    }
    if req.quality == 0 || req.quality > 100 {
        return Err(anyhow!("quality must be 1..=100"));
    }
    if let Some(parent) = req.destination.parent() {
        if !parent.exists() {
            return Err(anyhow!("destination directory does not exist"));
        }
    }

    // Resolve source photo path from catalog.
    let src_path: PathBuf = {
        let conn = catalog::open(library_root)?;
        let rel: String = conn
            .query_row(
                "SELECT rel_path FROM photos WHERE id = ?1 AND deleted_at IS NULL",
                params![&req.photo_id],
                |r| r.get(0),
            )
            .context("photo not found")?;
        library_root.join(rel)
    };

    let img = image::open(&src_path).with_context(|| format!("open {}", src_path.display()))?;
    let stack = get_stack(library_root, &req.photo_id).unwrap_or_default();
    let applied_ops = stack.len();
    let mut out = apply_stack(img, &stack)?;

    if let Some(max_edge) = req.max_edge {
        use image::imageops::FilterType;
        let (w, h) = (out.width(), out.height());
        if w.max(h) > max_edge {
            let (nw, nh) = if w >= h {
                (max_edge, (h as f32 * max_edge as f32 / w as f32).round() as u32)
            } else {
                ((w as f32 * max_edge as f32 / h as f32).round() as u32, max_edge)
            };
            out = out.resize_exact(nw.max(1), nh.max(1), FilterType::Lanczos3);
        }
    }

    let rgb = out.to_rgb8();
    let (width, height) = rgb.dimensions();

    let file = File::create(&req.destination)
        .with_context(|| format!("create {}", req.destination.display()))?;
    let mut w = BufWriter::new(file);
    let encoder = JpegEncoder::new_with_quality(&mut w, req.quality);
    encoder
        .write_image(rgb.as_raw(), width, height, ColorType::Rgb8.into())
        .context("encode JPEG")?;

    let bytes = std::fs::metadata(&req.destination).map(|m| m.len()).unwrap_or(0);

    Ok(ExportReport {
        written_path: req.destination.to_string_lossy().into_owned(),
        bytes,
        width,
        height,
        applied_ops,
    })
}
