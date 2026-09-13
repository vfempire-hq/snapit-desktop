// storage/thumbs.rs — thumbnail generation.
//
// Thumbs live inside the library at `_snapit/thumbs/<hash-shard>/<hash>-<size>.jpg`.
// We produce three sizes at generation time (128, 256, 1024) so the grid, the
// full-view, and the print preview are all instant. Format is JPEG q=85 for
// broad decoder support — the .avif upgrade lands in R·02 once we bundle libavif.

use anyhow::{Context, Result};
use image::imageops::FilterType;
use std::path::{Path, PathBuf};

use crate::catalog;

pub const THUMB_SIZES: &[u32] = &[128, 256, 1024];

fn thumbs_root(library_root: &Path) -> PathBuf {
    library_root.join(catalog::CATALOG_DIR).join("thumbs")
}

fn thumb_path(library_root: &Path, content_hash: &str, size: u32) -> PathBuf {
    let shard = &content_hash[..2];
    thumbs_root(library_root)
        .join(shard)
        .join(format!("{}-{}.jpg", content_hash, size))
}

pub fn ensure_thumbs(library_root: &Path, photo_path: &Path, content_hash: &str) -> Result<()> {
    // Fast return if all sizes already exist.
    if THUMB_SIZES
        .iter()
        .all(|s| thumb_path(library_root, content_hash, *s).exists())
    {
        return Ok(());
    }

    let img = image::open(photo_path).with_context(|| format!("decode {:?}", photo_path))?;
    let (src_w, src_h) = (img.width(), img.height());

    for &size in THUMB_SIZES {
        let out_path = thumb_path(library_root, content_hash, size);
        if out_path.exists() {
            continue;
        }
        std::fs::create_dir_all(out_path.parent().unwrap())?;
        let (target_w, target_h) = fit(src_w, src_h, size);
        let resized = img.resize(target_w, target_h, FilterType::Lanczos3);
        // Write to a temp then rename so we never leave a half-written thumb.
        let tmp = out_path.with_extension("jpg.tmp");
        {
            let mut f = std::fs::File::create(&tmp)?;
            let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut f, 85);
            encoder.encode(
                resized.to_rgb8().as_raw(),
                resized.width(),
                resized.height(),
                image::ExtendedColorType::Rgb8,
            )?;
        }
        std::fs::rename(&tmp, &out_path)?;
    }
    Ok(())
}

fn fit(w: u32, h: u32, max: u32) -> (u32, u32) {
    if w == 0 || h == 0 {
        return (max, max);
    }
    if w >= h {
        let target_w = max.min(w);
        let target_h = (target_w as f64 * h as f64 / w as f64).round() as u32;
        (target_w, target_h.max(1))
    } else {
        let target_h = max.min(h);
        let target_w = (target_h as f64 * w as f64 / h as f64).round() as u32;
        (target_w.max(1), target_h)
    }
}

pub fn thumb_asset_path(library_root: &Path, content_hash: &str, size: u32) -> PathBuf {
    thumb_path(library_root, content_hash, size)
}
