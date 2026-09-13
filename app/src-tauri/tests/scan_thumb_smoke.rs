// End-to-end smoke: create a real library folder with real JPEGs, run the
// scan + thumb pipeline the app uses, assert the thumbs are valid JPEGs and
// live where thumb_ensure_asset_path says they will.
//
// This is what should have been running before I called anything "beta".

use std::fs;
use std::path::PathBuf;

use snapit_lib::catalog;
use snapit_lib::storage;

fn make_jpeg(path: &PathBuf, w: u32, h: u32, r: u8, g: u8, b: u8) {
    use image::{ImageBuffer, Rgb};
    let img: ImageBuffer<Rgb<u8>, _> = ImageBuffer::from_pixel(w, h, Rgb([r, g, b]));
    img.save(path).unwrap();
}

#[test]
fn full_scan_and_thumb_pipeline() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().to_path_buf();

    // Create three test JPEGs at realistic sizes.
    make_jpeg(&root.join("a.jpg"), 3200, 2400, 200, 50, 40);
    make_jpeg(&root.join("b.jpg"), 1600, 1200, 30, 200, 80);
    fs::create_dir_all(root.join("nested")).unwrap();
    make_jpeg(&root.join("nested").join("c.jpeg"), 800, 600, 50, 60, 220);

    // Scan.
    let ingested = storage::local::scan(&root).expect("scan failed");
    assert_eq!(ingested, 3, "expected 3 photos indexed, got {}", ingested);

    // Get catalog + iterate rows.
    let rows = catalog::recent_filtered(&root, 100, 0).expect("recent failed");
    assert_eq!(rows.len(), 3, "expected 3 rows in recent");

    for row in &rows {
        // path_and_hash_by_id gives us rel_path + content_hash
        let (rel, hash) = catalog::path_and_hash_by_id(&root, &row.id).expect("path_and_hash");
        assert!(!hash.is_empty(), "content_hash must not be empty for {}", rel);

        let abs = root.join(&rel);
        assert!(abs.exists(), "source file missing: {:?}", abs);

        // Generate thumbs.
        storage::thumbs::ensure_thumbs(&root, &abs, &hash).expect("ensure_thumbs failed");

        // Verify each size exists and is a valid JPEG.
        for &size in storage::thumbs::THUMB_SIZES {
            let thumb = storage::thumbs::thumb_asset_path(&root, &hash, size);
            assert!(thumb.exists(), "thumb missing: {:?}", thumb);
            let bytes = fs::read(&thumb).unwrap();
            assert!(bytes.len() > 100, "thumb suspiciously small: {}", bytes.len());
            // JPEG magic 0xFF 0xD8 0xFF
            assert_eq!(&bytes[..3], &[0xFF, 0xD8, 0xFF], "not a JPEG: {:?}", thumb);
            // Decode round-trip: image crate should read our own thumb back.
            let decoded = image::open(&thumb).expect("thumb won't decode");
            let (tw, th) = (decoded.width(), decoded.height());
            assert!(tw > 0 && th > 0);
            assert!(tw.max(th) <= size, "thumb {}x{} exceeds requested {}", tw, th, size);
        }
    }
}

#[test]
fn heic_indexed_but_thumb_fails_gracefully() {
    // HEIC files are indexed (they're valid photos with EXIF), but the pure-Rust
    // image crate can't decode them. The bug I'm guarding against is: an unhandled
    // panic in the thumb pipeline that kills the frontend's next request.
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().to_path_buf();

    // Write a file with the right extension but garbage bytes.
    fs::write(root.join("fake.heic"), b"NOT A REAL HEIC FILE").unwrap();

    let ingested = storage::local::scan(&root).ok();
    // scan itself must not panic even if it can't get EXIF/dimensions.
    let _ = ingested;

    // If ANY row landed in the catalog for this file, ensure_thumbs must not
    // panic — it should return an Err the frontend can present as an X.
    if let Ok(rows) = catalog::recent_filtered(&root, 10, 0) {
        for row in rows {
            if let Ok((rel, hash)) = catalog::path_and_hash_by_id(&root, &row.id) {
                let abs = root.join(&rel);
                // MUST NOT panic. Result may be Err — that's fine.
                let _ = storage::thumbs::ensure_thumbs(&root, &abs, &hash);
            }
        }
    }
}
