// catalog/mod.rs — SQLite catalog. Lives inside the library folder.
//
// Design note: the catalog is a PORTABLE artefact. Copy the library dir to
// another machine and open it — SnapIT reads the same catalog and picks up
// exactly where you left off. That is the moat: your data is not locked in
// our servers because there is no server.

use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use serde::Serialize;
use std::path::{Path, PathBuf};

pub const SCHEMA_VERSION: u32 = 2;
pub const CATALOG_DIR: &str = "_snapit";
pub const CATALOG_FILE: &str = "catalog.sqlite";

pub fn catalog_path(library_root: &Path) -> PathBuf {
    library_root.join(CATALOG_DIR).join(CATALOG_FILE)
}

pub fn open(library_root: &Path) -> Result<Connection> {
    let path = catalog_path(library_root);
    let conn = Connection::open(&path).with_context(|| format!("open catalog {:?}", path))?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    Ok(conn)
}

pub fn open_or_init(library_root: &Path) -> Result<()> {
    let dir = library_root.join(CATALOG_DIR);
    std::fs::create_dir_all(&dir).with_context(|| format!("mkdir {:?}", dir))?;
    let conn = open(library_root)?;
    init_schema(&conn)?;
    Ok(())
}

fn migrate_1_to_2(conn: &Connection) -> Result<()> {
    // v2 adds XMP columns. Use ALTER TABLE ADD COLUMN; SQLite doesn't have
    // IF NOT EXISTS on ADD COLUMN so we swallow the "duplicate column" error.
    let alters = &[
        "ALTER TABLE photos ADD COLUMN xmp_rating INTEGER",
        "ALTER TABLE photos ADD COLUMN xmp_label TEXT",
        "ALTER TABLE photos ADD COLUMN xmp_caption TEXT",
        "ALTER TABLE photos ADD COLUMN xmp_keywords TEXT",
        "CREATE INDEX IF NOT EXISTS idx_photos_rating ON photos (xmp_rating)",
    ];
    for sql in alters {
        let _ = conn.execute_batch(sql);
    }
    Ok(())
}

fn current_version(conn: &Connection) -> Result<u32> {
    let v: Option<String> = conn
        .query_row("SELECT value FROM meta WHERE key = 'schema_version'", [], |r| r.get(0))
        .ok();
    Ok(v.and_then(|s| s.parse().ok()).unwrap_or(0))
}

fn init_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS meta (
            key   TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS photos (
            id            TEXT PRIMARY KEY,           -- ULID/uuidv7
            rel_path      TEXT NOT NULL UNIQUE,       -- path relative to library root
            content_hash  TEXT NOT NULL,              -- blake3 of the file bytes
            perceptual    TEXT,                       -- 64-bit dhash hex; NULL until AI stage
            width         INTEGER,
            height        INTEGER,
            byte_size     INTEGER NOT NULL,
            mtime         INTEGER NOT NULL,           -- filesystem mtime seconds
            taken_at      TEXT,                       -- EXIF DateTimeOriginal (ISO-8601)
            camera_make   TEXT,
            camera_model  TEXT,
            orientation   INTEGER,                    -- EXIF orientation 1..8
            lat           REAL,
            lon           REAL,
            xmp_rating    INTEGER,                    -- 0..5 stars from XMP sidecar
            xmp_label     TEXT,                       -- color label (Red/Blue/…)
            xmp_caption   TEXT,                       -- caption / description
            xmp_keywords  TEXT,                       -- CSV of hierarchical keywords
            imported_at   TEXT NOT NULL,
            deleted_at    TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_photos_rating ON photos (xmp_rating);
        CREATE INDEX IF NOT EXISTS idx_photos_taken_at ON photos (taken_at);
        CREATE INDEX IF NOT EXISTS idx_photos_hash    ON photos (content_hash);
        CREATE INDEX IF NOT EXISTS idx_photos_perceptual ON photos (perceptual);

        CREATE TABLE IF NOT EXISTS tags (
            id     TEXT PRIMARY KEY,
            name   TEXT NOT NULL UNIQUE COLLATE NOCASE
        );
        CREATE TABLE IF NOT EXISTS photo_tags (
            photo_id TEXT NOT NULL REFERENCES photos(id) ON DELETE CASCADE,
            tag_id   TEXT NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
            PRIMARY KEY (photo_id, tag_id)
        );

        CREATE TABLE IF NOT EXISTS faces (
            id           TEXT PRIMARY KEY,
            photo_id     TEXT NOT NULL REFERENCES photos(id) ON DELETE CASCADE,
            cluster_id   TEXT,                        -- filled after clustering
            embedding    BLOB NOT NULL,               -- 512-d f32 vector, little-endian
            bbox_x       REAL NOT NULL,
            bbox_y       REAL NOT NULL,
            bbox_w       REAL NOT NULL,
            bbox_h       REAL NOT NULL,
            score        REAL NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_faces_photo ON faces (photo_id);

        CREATE TABLE IF NOT EXISTS clusters (
            id      TEXT PRIMARY KEY,
            label   TEXT,                             -- user-assigned name; NULL = unnamed
            hidden  INTEGER NOT NULL DEFAULT 0
        );

        CREATE TABLE IF NOT EXISTS edits (
            photo_id     TEXT PRIMARY KEY REFERENCES photos(id) ON DELETE CASCADE,
            stack_json   TEXT NOT NULL,               -- JSON array of ordered edit ops
            updated_at   TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS scan_log (
            id           INTEGER PRIMARY KEY AUTOINCREMENT,
            started_at   TEXT NOT NULL,
            ended_at     TEXT,
            files_seen   INTEGER,
            files_added  INTEGER,
            files_moved  INTEGER,
            files_deleted INTEGER,
            note         TEXT
        );
        "#,
    )?;
    // Idempotent forward migrations for pre-existing catalogs.
    let cur = current_version(conn).unwrap_or(0);
    if cur < 2 {
        migrate_1_to_2(conn)?;
    }
    conn.execute(
        "INSERT OR REPLACE INTO meta (key, value) VALUES ('schema_version', ?1)",
        params![SCHEMA_VERSION.to_string()],
    )?;
    Ok(())
}

pub fn photo_count(library_root: &Path) -> Result<u64> {
    let conn = open(library_root)?;
    let n: u64 = conn.query_row(
        "SELECT COUNT(*) FROM photos WHERE deleted_at IS NULL",
        [],
        |r| r.get(0),
    )?;
    Ok(n)
}

#[derive(Serialize)]
pub struct PhotoRow {
    pub id: String,
    pub path: String,
    pub taken_at: Option<String>,
    pub width: i64,
    pub height: i64,
    pub xmp_rating: Option<i32>,
}

pub fn path_and_hash_by_id(library_root: &Path, photo_id: &str) -> Result<(String, String)> {
    let conn = open(library_root)?;
    let row = conn.query_row(
        "SELECT rel_path, content_hash FROM photos WHERE id = ?1",
        params![photo_id],
        |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
    )?;
    Ok(row)
}

pub fn recent(library_root: &Path, limit: u32) -> Result<Vec<PhotoRow>> {
    let conn = open(library_root)?;
    let mut stmt = conn.prepare(
        "SELECT id, rel_path, taken_at, COALESCE(width,0), COALESCE(height,0), xmp_rating
         FROM photos
         WHERE deleted_at IS NULL
         ORDER BY COALESCE(taken_at, imported_at) DESC
         LIMIT ?1",
    )?;
    let rows = stmt
        .query_map(params![limit], |r| {
            Ok(PhotoRow {
                id: r.get(0)?,
                path: r.get(1)?,
                taken_at: r.get(2)?,
                width: r.get(3)?,
                height: r.get(4)?,
                xmp_rating: r.get(5)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

pub struct UpsertPhoto<'a> {
    pub id: &'a str,
    pub rel_path: &'a str,
    pub content_hash: &'a str,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub byte_size: u64,
    pub mtime: i64,
    pub taken_at: Option<String>,
    pub camera_make: Option<String>,
    pub camera_model: Option<String>,
    pub orientation: Option<u32>,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
    pub xmp_rating: Option<i32>,
    pub xmp_label: Option<String>,
    pub xmp_caption: Option<String>,
    pub xmp_keywords: Option<String>,
    pub imported_at: String,
}

pub fn upsert(conn: &Connection, p: &UpsertPhoto<'_>) -> Result<()> {
    conn.execute(
        r#"
        INSERT INTO photos (
            id, rel_path, content_hash, width, height, byte_size, mtime,
            taken_at, camera_make, camera_model, orientation, lat, lon,
            xmp_rating, xmp_label, xmp_caption, xmp_keywords,
            imported_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)
        ON CONFLICT(rel_path) DO UPDATE SET
            content_hash = excluded.content_hash,
            width        = excluded.width,
            height       = excluded.height,
            byte_size    = excluded.byte_size,
            mtime        = excluded.mtime,
            taken_at     = excluded.taken_at,
            camera_make  = excluded.camera_make,
            camera_model = excluded.camera_model,
            orientation  = excluded.orientation,
            lat          = excluded.lat,
            lon          = excluded.lon,
            xmp_rating   = excluded.xmp_rating,
            xmp_label    = excluded.xmp_label,
            xmp_caption  = excluded.xmp_caption,
            xmp_keywords = excluded.xmp_keywords,
            deleted_at   = NULL
        "#,
        params![
            p.id,
            p.rel_path,
            p.content_hash,
            p.width,
            p.height,
            p.byte_size as i64,
            p.mtime,
            p.taken_at,
            p.camera_make,
            p.camera_model,
            p.orientation,
            p.lat,
            p.lon,
            p.xmp_rating,
            p.xmp_label,
            p.xmp_caption,
            p.xmp_keywords,
            p.imported_at,
        ],
    )?;
    Ok(())
}

/// Mark rel_paths that WERE in the catalog but are no longer on disk as
/// soft-deleted. Called at the end of a scan pass.
pub fn mark_deleted_except(conn: &Connection, seen: &[String]) -> Result<usize> {
    // SQLite doesn't take arrays; build an ephemeral in-memory table.
    conn.execute("CREATE TEMP TABLE IF NOT EXISTS seen_paths (rel_path TEXT PRIMARY KEY)", [])?;
    conn.execute("DELETE FROM seen_paths", [])?;
    let now = chrono::Utc::now().to_rfc3339();
    let tx_conn = conn;
    {
        let mut stmt = tx_conn.prepare("INSERT OR IGNORE INTO seen_paths (rel_path) VALUES (?1)")?;
        for p in seen {
            let _ = stmt.execute(params![p]);
        }
    }
    let n = tx_conn.execute(
        "UPDATE photos SET deleted_at = ?1
         WHERE deleted_at IS NULL
           AND rel_path NOT IN (SELECT rel_path FROM seen_paths)",
        params![now],
    )?;
    Ok(n)
}
