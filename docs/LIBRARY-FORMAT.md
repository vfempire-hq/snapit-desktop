# SnapIT — library format specification

**Version:** 1 (schema_version = 1)
**Status:** Frozen for R·01. Any breaking change bumps the major schema version and ships a one-shot migration.

Why this document exists: **your library outlives our company.** This file is the guarantee that anyone — including a competitor — can read a SnapIT library and reconstruct it. There is no vendor lock-in.

## On-disk layout

```
<your library root>/
├── <your original folders and photos, untouched>
│   ├── 2024/
│   │   └── italy/
│   │       ├── IMG_4523.jpg
│   │       └── IMG_4523.xmp        ← non-destructive edits (XMP sidecar)
│   └── ...
└── _snapit/                        ← everything SnapIT adds
    ├── catalog.sqlite              ← the catalog (see schema below)
    ├── catalog.sqlite-wal          ← SQLite WAL (transient)
    ├── catalog.sqlite-shm          ← SQLite shared memory (transient)
    ├── thumbs/                     ← generated thumbnails (128 / 256 / 1024 px)
    │   └── <content-hash-first-2>/<content-hash>.avif
    ├── models/                     ← optional model cache (offloaded from app dir)
    └── logs/                       ← scan + import logs, capped at 20 MB
```

**Rule:** the `_snapit/` directory is regenerable from the originals + sidecars.
Delete it and re-scan and you get an equivalent catalog. That is on purpose.

## SQLite schema (v1)

The full DDL lives in [`app/src-tauri/src/catalog/mod.rs`](../app/src-tauri/src/catalog/mod.rs). Summary:

- `meta(key, value)` — schema version + housekeeping.
- `photos(id, rel_path, content_hash, ...)` — one row per file. Unique key on `rel_path`. `deleted_at` is soft-delete.
- `tags` + `photo_tags` — user tags.
- `faces` + `clusters` — face detection + clustering.
- `edits` — one row per photo with a JSON stack of non-destructive edit operations.
- `scan_log` — audit trail of every scan pass.

## Edit stack format

`edits.stack_json` is a JSON array of ops applied in order. Every op has a stable name so future SnapIT versions can honour older stacks.

```json
[
  { "op": "crop",     "x": 0.05, "y": 0.10, "w": 0.90, "h": 0.80 },
  { "op": "rotate",   "deg": 0 },
  { "op": "expose",   "ev": 0.4 },
  { "op": "contrast", "amount": 0.15 },
  { "op": "sat",      "amount": -0.10 }
]
```

Ops are pure — same input + same stack = same output pixel-for-pixel. No random seeds.

## Face embeddings

`faces.embedding` is a 512-dimensional float32 vector, stored little-endian, no compression. That's 2 KB per face. For a 15k-photo library with an average of 1.2 faces per photo, this table is ~36 MB — negligible.

Clustering happens locally with HDBSCAN; `faces.cluster_id` is a foreign key into `clusters`, which the user can name.

## Portable-library contract

If you take the library folder to another SnapIT install:
- All photos + sidecars are already there.
- The catalog opens read-only until the app verifies schema version.
- A background rescan reconciles any drift (files moved, added, deleted).

If you take the library folder to a competitor:
- Photos + XMPs still work — everyone understands XMP.
- The `_snapit/` folder is optional; deleting it does no damage.
- The SQLite schema is documented in this file so a competitor can adopt it.

That is the whole point.
