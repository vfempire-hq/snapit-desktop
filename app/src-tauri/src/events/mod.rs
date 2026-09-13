// events/mod.rs — event clustering.
//
// The R·02 story: photos already know when they were taken and where. SnapIT
// reads that and files them into events the way a human would remember them —
// the trip, the birthday, the site visit. Not folders, not a timeline scroll —
// events.
//
// Algorithm (v1, deterministic):
//   1. Sort all non-deleted media by taken_at ascending.
//   2. Walk the sorted list. Start a new event whenever the gap between two
//      consecutive shots exceeds GAP_HOURS.
//   3. Refine: if two adjacent events share a place tag AND a camera AND the
//      gap between them is under REFINE_HOURS, merge them.
//   4. Discard singletons (unless the singleton is a video >= 30s).
//   5. Compute cover photo per event: highest xmp_rating, tie-break by
//      centroid position.
//
// This runs after every scan. It is idempotent: re-running on the same catalog
// produces the same events (edited=1 events are preserved untouched — user's
// manual name / merge / split intent wins).

use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::params;
use std::path::Path;

use crate::catalog;

const GAP_HOURS: i64 = 8;     // > this many hours between shots = new event
const REFINE_HOURS: i64 = 24; // merge same-place events within this window
const MIN_EVENT_SIZE: usize = 2;

#[derive(Debug, Clone)]
struct MediaRow {
    id: String,
    taken_at: DateTime<Utc>,
    lat: Option<f64>,
    lon: Option<f64>,
    camera_model: Option<String>,
    xmp_rating: Option<i32>,
    kind: String,
    duration_ms: Option<i64>,
}

pub fn recluster(library_root: &Path) -> Result<usize> {
    let conn = catalog::open(library_root)?;

    // Only rebuild non-edited events. User-edited names/merges/splits stay.
    conn.execute(
        "DELETE FROM event_photos WHERE event_id IN (SELECT id FROM events WHERE edited = 0)",
        [],
    )?;
    conn.execute("DELETE FROM events WHERE edited = 0", [])?;

    // Pull every media row that has a taken_at.
    let mut rows: Vec<MediaRow> = Vec::new();
    {
        let mut stmt = conn.prepare(
            "SELECT id, taken_at, lat, lon, camera_model, xmp_rating, kind, duration_ms
             FROM photos
             WHERE deleted_at IS NULL AND taken_at IS NOT NULL
             ORDER BY taken_at ASC",
        )?;
        let mapped = stmt.query_map([], |r| {
            let taken_at_s: String = r.get(1)?;
            Ok(MediaRow {
                id: r.get(0)?,
                taken_at: DateTime::parse_from_rfc3339(&taken_at_s)
                    .map(|d| d.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
                lat: r.get(2).ok(),
                lon: r.get(3).ok(),
                camera_model: r.get(4).ok(),
                xmp_rating: r.get(5).ok(),
                kind: r.get::<_, String>(6).unwrap_or_else(|_| "photo".into()),
                duration_ms: r.get(7).ok(),
            })
        })?;
        for row in mapped.flatten() {
            rows.push(row);
        }
    }

    if rows.is_empty() {
        return Ok(0);
    }

    // Also exclude rows that already belong to an EDITED event — keep them
    // in the manual bucket.
    let mut edited_photo_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
    {
        let mut stmt = conn.prepare(
            "SELECT ep.photo_id FROM event_photos ep
             JOIN events e ON e.id = ep.event_id
             WHERE e.edited = 1",
        )?;
        let mapped = stmt.query_map([], |r| r.get::<_, String>(0))?;
        for pid in mapped.flatten() {
            edited_photo_ids.insert(pid);
        }
    }
    rows.retain(|r| !edited_photo_ids.contains(&r.id));

    // Step 1+2: split into candidate events by GAP_HOURS.
    let mut candidates: Vec<Vec<MediaRow>> = Vec::new();
    let mut current: Vec<MediaRow> = Vec::new();
    for row in rows {
        if let Some(last) = current.last() {
            let gap = row.taken_at.signed_duration_since(last.taken_at);
            if gap.num_hours() > GAP_HOURS {
                if !current.is_empty() {
                    candidates.push(std::mem::take(&mut current));
                }
            }
        }
        current.push(row);
    }
    if !current.is_empty() {
        candidates.push(current);
    }

    // Step 3: merge adjacent same-place / same-camera events within REFINE_HOURS.
    let mut refined: Vec<Vec<MediaRow>> = Vec::new();
    for cand in candidates {
        if let Some(prev) = refined.last() {
            if let (Some(prev_last), Some(cand_first)) = (prev.last(), cand.first()) {
                let gap = cand_first.taken_at.signed_duration_since(prev_last.taken_at);
                let same_place = places_close(prev_last, cand_first);
                let same_camera = prev_last.camera_model == cand_first.camera_model;
                if gap.num_hours() <= REFINE_HOURS && same_place && same_camera {
                    refined.last_mut().unwrap().extend(cand);
                    continue;
                }
            }
        }
        refined.push(cand);
    }

    // Step 4: discard singletons unless they're a non-trivial video.
    refined.retain(|e| {
        if e.len() >= MIN_EVENT_SIZE {
            return true;
        }
        e.iter().any(|r| r.kind == "video" && r.duration_ms.unwrap_or(0) >= 30_000)
    });

    // Step 5: persist each event + members. Confidence is a rough proxy for
    // "how tightly this event clustered" — used by the UI to badge auto-events.
    let now = Utc::now().to_rfc3339();
    let mut written = 0;
    for e in refined {
        let start = e.first().map(|r| r.taken_at).unwrap_or_else(Utc::now);
        let end = e.last().map(|r| r.taken_at).unwrap_or_else(Utc::now);
        let title = generate_title(&start, &end, &e);
        let cover = pick_cover(&e);
        let confidence = cluster_confidence(&e);
        let place = dominant_place_name(&e);

        let event_id = uuid::Uuid::now_v7().to_string();
        conn.execute(
            r#"INSERT INTO events
               (id, title, start_at, end_at, place, confidence, cover_photo, created_at, edited)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 0)"#,
            params![
                event_id,
                title,
                start.to_rfc3339(),
                end.to_rfc3339(),
                place,
                confidence,
                cover,
                now,
            ],
        )?;
        for m in &e {
            conn.execute(
                "INSERT OR IGNORE INTO event_photos (event_id, photo_id) VALUES (?1, ?2)",
                params![event_id, m.id],
            )?;
        }
        written += 1;
    }
    Ok(written)
}

fn places_close(a: &MediaRow, b: &MediaRow) -> bool {
    match (a.lat, a.lon, b.lat, b.lon) {
        (Some(la1), Some(lo1), Some(la2), Some(lo2)) => haversine_km(la1, lo1, la2, lo2) < 5.0,
        _ => true, // no GPS on either side → assume same-place, don't hard-split
    }
}

fn haversine_km(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let to_rad = std::f64::consts::PI / 180.0;
    let dlat = (lat2 - lat1) * to_rad;
    let dlon = (lon2 - lon1) * to_rad;
    let a = (dlat / 2.0).sin().powi(2)
        + (lat1 * to_rad).cos() * (lat2 * to_rad).cos() * (dlon / 2.0).sin().powi(2);
    2.0 * 6371.0 * a.sqrt().atan2((1.0 - a).sqrt())
}

fn generate_title(start: &DateTime<Utc>, end: &DateTime<Utc>, _rows: &[MediaRow]) -> String {
    // v1: month + day range. v2 will incorporate place names and event type.
    let same_day = start.date_naive() == end.date_naive();
    if same_day {
        start.format("%B %-d, %Y").to_string()
    } else {
        format!("{} – {}", start.format("%B %-d"), end.format("%-d, %Y"))
    }
}

fn pick_cover(rows: &[MediaRow]) -> Option<String> {
    let mut best: Option<&MediaRow> = None;
    let mut best_score = i32::MIN;
    for r in rows {
        // Prefer highest rating; break ties with the median-time photo.
        let rating = r.xmp_rating.unwrap_or(0);
        let score = rating * 10;
        if score > best_score {
            best_score = score;
            best = Some(r);
        }
    }
    // If nothing rated, pick the median-time item.
    if best_score <= 0 && !rows.is_empty() {
        best = Some(&rows[rows.len() / 2]);
    }
    best.map(|r| r.id.clone())
}

fn cluster_confidence(rows: &[MediaRow]) -> f64 {
    // Fraction of shots that share the dominant camera_model.
    let mut by_model = std::collections::HashMap::<String, u32>::new();
    for r in rows {
        let k = r.camera_model.clone().unwrap_or_else(|| "unknown".into());
        *by_model.entry(k).or_insert(0) += 1;
    }
    let total = rows.len() as f64;
    let top = by_model.values().max().copied().unwrap_or(0) as f64;
    (top / total).min(1.0)
}

fn dominant_place_name(_rows: &[MediaRow]) -> Option<String> {
    // v1 stub — needs a reverse-geocoding pass. R·02-late slots that in.
    None
}
