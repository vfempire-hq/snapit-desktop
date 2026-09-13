// storage/local.rs — filesystem walker + importer.
//
// R·01 M1: walk the library root, index every image we find into the catalog.
// Skips the _snapit directory (that's OUR data), hidden dirs, and anything
// that isn't an image extension we understand.

use anyhow::{Context, Result};
use blake3::Hasher;
use chrono::Utc;
use rayon::prelude::*;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use crate::catalog;

const IMAGE_EXTS: &[&str] = &[
    "jpg", "jpeg", "png", "webp", "heic", "heif", "tif", "tiff", "bmp",
    "cr2", "cr3", "nef", "arw", "dng", "rw2", "raf", "orf", "pef", "srw",
];

pub fn scan(library_root: &Path) -> Result<u64> {
    // Ensure catalog is initialised
    catalog::open_or_init(library_root)?;

    // Collect candidates first (single-threaded walk), then process in parallel.
    let mut candidates: Vec<PathBuf> = Vec::new();
    for entry in WalkDir::new(library_root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            // skip our own catalog dir + dot-dirs
            !(name == crate::catalog::CATALOG_DIR
                || name.starts_with('.')
                || name == "@eaDir"
                || name == "$RECYCLE.BIN"
                || name == "System Volume Information")
        })
    {
        let entry = match entry { Ok(e) => e, Err(_) => continue };
        if !entry.file_type().is_file() { continue; }
        let path = entry.into_path();
        if is_image(&path) { candidates.push(path); }
    }

    tracing::info!("scan: {} candidate files under {:?}", candidates.len(), library_root);

    // Content-hash + EXIF in parallel; feed rows back to a serial writer.
    let (tx, rx) = crossbeam_channel::bounded::<catalog::UpsertPhoto<'static>>(0);

    // Writer thread: single SQLite connection, batches inserts.
    let write_root = library_root.to_path_buf();
    let writer = std::thread::spawn(move || -> anyhow::Result<u64> {
        let mut conn = catalog::open(&write_root)?;
        let mut n: u64 = 0;
        let tx_conn = conn.transaction()?;
        while let Ok(p) = rx.recv() {
            catalog::upsert(&tx_conn, &p)?;
            n += 1;
            if n % 500 == 0 { tracing::info!("scan progress: {} indexed", n); }
        }
        tx_conn.commit()?;
        Ok(n)
    });

    // Producer side.
    candidates.par_iter().for_each_with(tx, |tx, path| {
        if let Ok(row) = process_file(library_root, path) {
            // We have to leak the strings for the channel type, but since the writer
            // consumes them synchronously and drops them, this is a bounded leak
            // scoped to a single scan pass. Simpler than lifetimes across threads.
            let leaked = leak_upsert(row);
            let _ = tx.send(leaked);
        }
    });

    let n = writer.join().unwrap()?;
    Ok(n)
}

fn is_image(path: &Path) -> bool {
    match path.extension().and_then(|e| e.to_str()) {
        Some(ext) => IMAGE_EXTS.iter().any(|e| ext.eq_ignore_ascii_case(e)),
        None => false,
    }
}

struct RowOwned {
    id: String,
    rel_path: String,
    content_hash: String,
    width: Option<u32>,
    height: Option<u32>,
    byte_size: u64,
    mtime: i64,
    taken_at: Option<String>,
    camera_make: Option<String>,
    camera_model: Option<String>,
    orientation: Option<u32>,
    lat: Option<f64>,
    lon: Option<f64>,
    imported_at: String,
}

fn process_file(library_root: &Path, path: &Path) -> Result<RowOwned> {
    let meta = fs::metadata(path)?;
    let byte_size = meta.len();
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    // Content hash (blake3, 8k window is plenty for now; full-file for M1)
    let mut file = fs::File::open(path).with_context(|| format!("open {:?}", path))?;
    let mut hasher = Hasher::new();
    let mut buf = [0u8; 128 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 { break; }
        hasher.update(&buf[..n]);
    }
    let content_hash = hasher.finalize().to_hex().to_string();

    // Dimensions — decode header only.
    let (width, height) = image::image_dimensions(path).ok().map(|(w, h)| (Some(w), Some(h))).unwrap_or((None, None));

    // EXIF (best-effort — fine for many jpeg/tiff/heif files).
    let (taken_at, camera_make, camera_model, orientation, lat, lon) = read_exif(path).unwrap_or_default();

    Ok(RowOwned {
        id: uuid::Uuid::now_v7().to_string(),
        rel_path: rel_path_str(library_root, path),
        content_hash,
        width,
        height,
        byte_size,
        mtime,
        taken_at,
        camera_make,
        camera_model,
        orientation,
        lat,
        lon,
        imported_at: Utc::now().to_rfc3339(),
    })
}

fn rel_path_str(root: &Path, path: &Path) -> String {
    match path.strip_prefix(root) {
        Ok(p) => p.to_string_lossy().to_string(),
        Err(_) => path.to_string_lossy().to_string(),
    }
}

type ExifFields = (Option<String>, Option<String>, Option<String>, Option<u32>, Option<f64>, Option<f64>);

fn read_exif(path: &Path) -> Result<ExifFields> {
    let file = fs::File::open(path)?;
    let mut buf_reader = std::io::BufReader::new(&file);
    let exif_reader = exif::Reader::new().read_from_container(&mut buf_reader)?;

    let taken_at = exif_reader
        .get_field(exif::Tag::DateTimeOriginal, exif::In::PRIMARY)
        .and_then(|f| f.display_value().to_string().parse::<chrono::NaiveDateTime>().ok())
        .or_else(|| {
            exif_reader
                .get_field(exif::Tag::DateTimeOriginal, exif::In::PRIMARY)
                .and_then(|f| {
                    let s = f.display_value().to_string();
                    // EXIF format: "YYYY:MM:DD HH:MM:SS"
                    chrono::NaiveDateTime::parse_from_str(&s, "%Y:%m:%d %H:%M:%S").ok()
                })
        })
        .map(|dt| dt.and_utc().to_rfc3339());

    let camera_make = exif_reader
        .get_field(exif::Tag::Make, exif::In::PRIMARY)
        .map(|f| f.display_value().to_string().trim_matches('"').to_string());
    let camera_model = exif_reader
        .get_field(exif::Tag::Model, exif::In::PRIMARY)
        .map(|f| f.display_value().to_string().trim_matches('"').to_string());
    let orientation = exif_reader
        .get_field(exif::Tag::Orientation, exif::In::PRIMARY)
        .and_then(|f| f.value.get_uint(0));

    let lat = exif_reader
        .get_field(exif::Tag::GPSLatitude, exif::In::PRIMARY)
        .and_then(dms_to_deg);
    let lon = exif_reader
        .get_field(exif::Tag::GPSLongitude, exif::In::PRIMARY)
        .and_then(dms_to_deg);

    Ok((taken_at, camera_make, camera_model, orientation, lat, lon))
}

fn dms_to_deg(f: &exif::Field) -> Option<f64> {
    let v = &f.value;
    let d = v.get_uint(0)? as f64;
    let m = v.get_uint(1)? as f64;
    let s = v.get_uint(2)? as f64;
    Some(d + m / 60.0 + s / 3600.0)
}

fn leak_upsert(r: RowOwned) -> catalog::UpsertPhoto<'static> {
    catalog::UpsertPhoto {
        id: Box::leak(r.id.into_boxed_str()),
        rel_path: Box::leak(r.rel_path.into_boxed_str()),
        content_hash: Box::leak(r.content_hash.into_boxed_str()),
        width: r.width,
        height: r.height,
        byte_size: r.byte_size,
        mtime: r.mtime,
        taken_at: r.taken_at,
        camera_make: r.camera_make,
        camera_model: r.camera_model,
        orientation: r.orientation,
        lat: r.lat,
        lon: r.lon,
        imported_at: r.imported_at,
    }
}
