// r02a_mvp_smoke.rs — end-to-end smoke test for the R·02a MVP surface.
//
// This isn't a WebView test — Xvfb + WebKitGTK is a rabbit hole for CI.
// What it DOES prove:
//   1. Profile bootstrap creates an Owner profile (M1 command surface)
//   2. Profile create + PIN + verify round-trip works (Argon2id path)
//   3. Filesystem scan discovers a real JPEG and writes to SQLite (M3)
//   4. catalog::recent_filtered returns the enriched PhotoRow shape
//      the mock UI now consumes (kind, camera, caption, duration_s,
//      content_hash) — this is the M3 command's payload
//   5. Thumbnail generation produces a decodable JPEG at the path the
//      hydrated command hands out (thumb_asset_path)
//
// Run:  cargo test --test r02a_mvp_smoke -- --nocapture

use image::{ImageBuffer, Rgb};
use snapit_lib::{catalog, profile, storage};
use tempfile::TempDir;

fn write_test_jpeg(dir: &std::path::Path, name: &str) -> std::path::PathBuf {
    let path = dir.join(name);
    // 400x300 solid gradient — big enough that thumbnail resize is real work,
    // small enough that the test runs in <1s per file.
    let mut img: ImageBuffer<Rgb<u8>, Vec<u8>> = ImageBuffer::new(400, 300);
    for (x, y, px) in img.enumerate_pixels_mut() {
        *px = Rgb([(x * 255 / 400) as u8, (y * 255 / 300) as u8, 128]);
    }
    img.save(&path).expect("write test jpeg");
    path
}

#[test]
fn profile_bootstrap_and_pin_roundtrip() {
    // profile:: writes to a real config dir — use HOME override so we
    // don't polute the user's config in a repeated test run.
    let scratch = TempDir::new().unwrap();
    // std::env::set_var("XDG_CONFIG_HOME", scratch.path());
    // dirs::config_dir() honours XDG_CONFIG_HOME; but the profile module
    // reads it on first use of store_path(), which caches nothing, so this
    // is safe to set here.
    unsafe { std::env::set_var("XDG_CONFIG_HOME", scratch.path()) };

    // Fresh install: no profiles yet.
    let before = profile::list_views();
    assert!(before.is_empty(), "fresh install should have zero profiles");

    // Bootstrap → one Owner profile appears.
    profile::bootstrap_if_empty().expect("bootstrap");
    let after = profile::list_views();
    assert_eq!(after.len(), 1, "bootstrap should create exactly one profile");
    assert_eq!(after[0].role, "owner");
    assert!(after[0].delete_forbidden, "owner cannot be deleted");
    assert!(!after[0].locked, "fresh owner has no PIN");

    // Bootstrap is idempotent — running twice must not duplicate.
    profile::bootstrap_if_empty().expect("bootstrap 2");
    let after2 = profile::list_views();
    assert_eq!(after2.len(), 1);

    // Create a second family member with a PIN.
    let input = profile::ProfileInput {
        name: "Elena".into(),
        initials: Some("E".into()),
        gradient: Some("gradient-e".into()),
        kids: false,
        role: Some("family".into()),
        content_restrictions: None,
    };
    let elena = profile::create_from_input(input, Some("4711".into()))
        .expect("create Elena with PIN");
    assert!(elena.locked, "Elena should be PIN-locked");
    assert_eq!(elena.name, "Elena");

    // Verify PIN — correct and incorrect.
    let ok = profile::verify_pin(&elena.id, "4711").expect("verify");
    assert!(ok, "correct PIN must verify");
    let wrong = profile::verify_pin(&elena.id, "9999").expect("verify wrong");
    assert!(!wrong, "wrong PIN must not verify");

    // Family Pack seat cap — creating a 6th profile should fail.
    for i in 3..=5 {
        profile::create_from_input(
            profile::ProfileInput { name: format!("Kid{i}"), ..Default::default() },
            None,
        )
        .expect("create kid");
    }
    let overflow = profile::create_from_input(
        profile::ProfileInput { name: "Extra".into(), ..Default::default() },
        None,
    );
    assert!(overflow.is_err(), "6th profile should be rejected");
    assert_eq!(overflow.unwrap_err().to_string(), "family_pack_full");

    // active_view() before + after set_active
    let owner_id = &after[0].id;
    profile::set_active(owner_id).expect("set active owner");
    let active = profile::active_view().expect("active present");
    assert_eq!(active.id, *owner_id);
}

#[test]
fn scan_and_hydrated_recent_returns_mock_ready_rows() {
    let lib = TempDir::new().unwrap();
    let lib_root = lib.path();
    catalog::open_or_init(lib_root).expect("init catalog");

    // Drop three test JPEGs at the library root.
    write_test_jpeg(lib_root, "a.jpg");
    write_test_jpeg(lib_root, "b.jpg");
    write_test_jpeg(lib_root, "c.jpg");

    // Run the real scanner — same code path the library_scan command uses.
    let count = storage::local::scan_with_progress(lib_root, None).expect("scan");
    assert_eq!(count, 3, "scan should discover all three JPEGs");

    // catalog::recent_filtered is what catalog_recent_hydrated calls into
    // — the same enriched PhotoRow shape the mock UI now expects.
    let rows = catalog::recent_filtered(lib_root, 100, 0).expect("recent");
    assert_eq!(rows.len(), 3, "recent must return all three");

    for row in &rows {
        assert_eq!(row.kind, "photo", "test JPEGs are photos, not videos");
        assert!(row.duration_s.is_none(), "photos have no duration");
        assert!(!row.content_hash.is_empty(), "content_hash must be populated");
        assert!(row.content_hash.len() >= 32, "blake3 hash is long");
    }
    let r0 = &rows[0];
    println!(
        "hydrated row 0: id={} kind={} camera={:?} caption={:?} rating={:?} hash={}",
        r0.id, r0.kind, r0.camera, r0.caption, r0.xmp_rating, r0.content_hash
    );

    // Thumbnail path resolution + generation should produce a real JPEG.
    // This mirrors the flow the mock's <img src> hits via convertFileSrc().
    let row = &rows[0];
    let abs = lib_root.join(&row.path);
    storage::thumbs::ensure_thumbs(lib_root, &abs, &row.content_hash).expect("ensure thumbs");
    let thumb_path = storage::thumbs::thumb_asset_path(lib_root, &row.content_hash, 256);
    assert!(thumb_path.exists(), "thumbnail must be materialised on disk");
    let thumb_bytes = std::fs::read(&thumb_path).expect("read thumb");
    assert!(
        thumb_bytes.starts_with(b"\xFF\xD8\xFF"),
        "thumbnail must be a valid JPEG (magic bytes)"
    );
    println!(
        "thumbnail at {} — {} bytes",
        thumb_path.display(),
        thumb_bytes.len()
    );
}
