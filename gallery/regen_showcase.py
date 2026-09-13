#!/usr/bin/env python3
# gallery/regen_showcase.py — build the customer-facing showcase.html
# from whatever's on disk under out/. Runs every time a batch finishes
# so the site grows as the volume batches land.
#
# Also mirrors gallery/out into the CF-served folder and calls the
# regeneration idempotently — safe to run repeatedly.

import json, os, shutil
from pathlib import Path
from html import escape

ROOT   = Path("/home/guardiansoftiktok/snapit-desktop")
OUT    = ROOT / "gallery" / "out"
SERVE  = ROOT / "purchase-backend" / "public" / "preview-x8f2r7"
GALLERY_DEST = SERVE / "gallery"
MOCK_ROOT = ROOT / "mock"

# Metadata hints keyed by filename fragment so the app's tiles get
# realistic place / people / camera / event / kind labels instead of
# random noise. Real Rust catalog will pull these from EXIF + ML.
PLACE_HINTS = {
    "malta": "Malta", "valletta": "Valletta", "comino": "Comino",
    "sicily": "Sicily", "taormina": "Taormina", "etna": "Etna",
    "alps": "Swiss Alps", "ski": "Swiss Alps", "italian": "Italian Alps",
    "rostock": "Rostock", "berlin": "Berlin", "leipzig": "Leipzig",
    "vietnam": "Vietnam", "bali": "Bali", "singapore": "Singapore",
    "tokyo": "Tokyo", "dubai": "Dubai", "spain": "Spain", "italy": "Italy",
    "germany": "Germany", "vineyard": "Tuscany", "yoga": "Bali",
    "mediterranean": "Malta", "cliff": "Comino",
}
EVENT_HINTS = {
    "malta-june":       "Malta trip · June",
    "sicily":           "Sicily road trip",
    "ski":              "Alps ski week",
    "family-christmas": "Christmas 2025",
    "family-reunion":   "Family reunion",
    "graduation":       "Graduation day",
    "baby":             "Baby's first year",
    "kitchen":          "Kitchen renovation",
    "halloween":        "Halloween",
    "new-year":         "New Year in Valletta",
    "backyard":         "Backyard BBQ",
    "anniversary":      "Anniversary dinner",
    "berlin":           "Berlin office move",
    "rostock":          "Rostock warehouse",
    "site-visit":       "Property viewing",
    "valletta-morning": "Malta trip · June",
    "cliff-yoga":       "Malta trip · June",
    "vineyard":         "Tuscany harvest",
    "mediterranean":    "Storm at sea",
}
CAMERA_BY_SECTION = {
    "hero":            "Nikon Z8",
    "highlights":      "Nikon Z8",
    "portrait":        "Fujifilm X-T5",
    "starred":         "Sony A7 IV",
    "still-life":      "Fujifilm X-T5",
    "place":           "Sony A7 IV",
    "kids-row":        "iPhone 15 Pro",
    "raw-technical":   "Nikon Z8",
    "video-still":     "DJI Mavic 3",
    "social-instagram":"iPhone 15 Pro",
    "social-tiktok":   "iPhone 15 Pro",
    "social-youtube":  "Sony A7 IV",
    "pets":            "iPhone 15 Pro",
    "action-sports":   "Sony A7 IV",
    "concert-festival":"iPhone 15 Pro",
    "hobby-diy":       "Fujifilm X-T5",
    "milestone":       "iPhone 15 Pro",
    "season-weather":  "Sony A7 IV",
    "nature-wildlife": "Nikon Z8",
    "selfies":         "iPhone 15 Pro",
}

# Aspect classification by filename convention — set from prompt files
ASPECTS_BY_SECTION = {
    "hero":            "landscape",  # 1344x768
    "highlights":      "landscape",
    "video-still":     "landscape",
    "place":           "landscape",
    "kids-row":        "landscape",
    "raw-technical":   "landscape",
    "social-youtube":  "landscape",
    "pets":            "landscape",
    "action-sports":   "landscape",
    "concert-festival":"landscape",
    "hobby-diy":       "landscape",
    "milestone":       "landscape",
    "season-weather":  "landscape",
    "nature-wildlife": "landscape",
    "portrait":        "portrait",   # 896x1152
    "selfies":         "portrait",
    "starred":         "square",     # 1024x1024
    "social-instagram":"square",
    "still-life":      "square",
    "social-tiktok":   "story",      # 720x1280
}

# Presentation order + human labels + descriptions
SECTIONS = [
    ("hero",             "Hero events",        "Billboard + featured-event archetypes. Every event a family remembers."),
    ("highlights",       "Highlights",         "Standout moments across the year — landscape, market, night, drone."),
    ("portrait",         "Portraits",          "People-row circular avatars. Environmental, unposed, film aesthetic."),
    ("starred",          "Starred",            "Detail shots photographers actually star. Top-10 row content."),
    ("video-still",      "Video stills",       "16:9 with motion blur — reads as a video frame not a photo."),
    ("social-instagram", "Instagram",          "Flatlay, wellness, food, travel. Extension-clean import + IG-chrome overlay preview."),
    ("social-tiktok",    "TikTok",             "9:16 vertical, phone-cam compression. Extension-clean import + TikTok-chrome overlay preview."),
    ("social-youtube",   "YouTube",            "Thumbnail-source frames — subject-to-camera. Extension-clean import + YT-chrome overlay preview."),
    ("place",            "Places",             "Architectural + landscape, no people. Places-row content."),
    ("still-life",       "Still-life",         "Object detail photography. Detail-shots row."),
    ("kids-row",         "Kids row",           "Kid-safe archetype content — auto-hidden mature clusters for Kids profiles."),
    ("pets",             "Pets",               "Every phone has hundreds of these — dogs, cats, and family with pets."),
    ("action-sports",    "Sport & action",     "Kids' football, cycling, gym, yoga, running — auto-clustered as activity events."),
    ("concert-festival", "Concerts & gigs",    "Stage lights, crowds, wristbands, festival food — event-detected via time + venue GPS."),
    ("hobby-diy",        "Hobbies & DIY",      "Workshop, gardening, pottery, painting — long-running hobby clusters."),
    ("milestone",        "Milestones",         "First-day-of-school, moving day, new car, engagement — auto-tagged as milestones."),
    ("season-weather",   "Seasons & weather",  "Autumn colour, first snow, storms, fog, sunrise, sunset — seasonal auto-albums."),
    ("nature-wildlife",  "Nature & wildlife",  "Birds, deer, macro flowers, mushrooms — content-tagged wildlife bucket."),
    ("selfies",          "Selfies",            "Mirror selfies, group selfies, travel selfies — big share of phone rolls."),
    ("raw-technical",    "RAW technical",      "Long-lens, wildlife, sport, macro, astro. Pro / Studio archetype."),
]

def mirror_gallery():
    """Idempotent copy of out/ into SERVE/gallery/."""
    GALLERY_DEST.mkdir(parents=True, exist_ok=True)
    for section_dir in OUT.iterdir():
        if not section_dir.is_dir():
            continue
        dest = GALLERY_DEST / section_dir.name
        dest.mkdir(exist_ok=True)
        for f in section_dir.iterdir():
            if f.is_file() and f.suffix == ".png":
                target = dest / f.name
                if not target.exists() or target.stat().st_size != f.stat().st_size:
                    shutil.copy2(f, target)

def scan_sections():
    """Returns { section_id: [ (name, url), ... ] }."""
    result = {}
    for section_id, _, _ in SECTIONS:
        d = OUT / section_id
        items = []
        if d.exists():
            for f in sorted(d.iterdir()):
                if f.is_file() and f.suffix == ".png":
                    items.append((f.stem, f"/preview-x8f2r7/gallery/{section_id}/{f.name}"))
        result[section_id] = items
    return result

def render_tile(section_id, name, url):
    """Single tile. Social tiles carry chrome markup that shows/hides via
    the global 'Show platform chrome' toggle. Chrome pulls the per-image
    metadata bank so the caption/handle/likes actually match the picture —
    exactly what the real app does when it reads metadata from the catalog."""
    aspect = ASPECTS_BY_SECTION.get(section_id, "landscape")
    if section_id.startswith("social-"):
        brand = section_id.split("-", 1)[1]
        meta = SOCIAL_META.get(name, SOCIAL_META_DEFAULT[brand])
        return f'''
        <div class="tile {aspect} social {brand}" onclick="lb(this)">
          <img src="{url}" alt="" loading="lazy">
          {chrome_overlay(brand, meta)}
          <div class="tile-cap"><div class="name">{escape(name.replace("-"," "))}</div></div>
        </div>'''
    return f'''
        <div class="tile {aspect}" onclick="lb(this)">
          <img src="{url}" alt="" loading="lazy">
          <div class="tile-cap"><div class="name">{escape(name.replace("-"," "))}</div></div>
        </div>'''

# Per-image metadata bank — real app will read these from catalog columns
# (xmp_caption / social_handle / social_likes / social_music / video_title).
# For the mock we hand-curate a caption per image so the chrome matches.
SOCIAL_META_DEFAULT = {
    "instagram": {"handle":"vincent.f", "caption":"", "hashtag":"", "likes":"1 284"},
    "tiktok":    {"handle":"vincent",   "caption":"", "hashtag":"", "likes":"128K",
                  "comments":"2.8K", "saves":"12K", "music":"original sound — @vincent"},
    "youtube":   {"title":"", "time":"4:12 / 12:38"},
}
SOCIAL_META = {
    # Instagram
    "ig-brunch":         {"handle":"vincent.f", "caption":"Sunday brunch, Maltese-style",   "hashtag":"#brunch #malta #ftira",       "likes":"2 143"},
    "ig-latte-art":      {"handle":"elena.f",   "caption":"morning ritual",                  "hashtag":"#coffee #latteart #slowmornings", "likes":"1 892"},
    "ig-outfit-flatlay": {"handle":"elena.f",   "caption":"summer packing done",             "hashtag":"#flatlay #ootd #summer",       "likes":"3 401"},
    "ig-travel-selfie":  {"handle":"vincent.f", "caption":"favourite people, favourite place","hashtag":"#malta #couplegoals",         "likes":"5 218"},
    "ig-yoga-beach":     {"handle":"elena.f",   "caption":"sunrise flow at Comino",          "hashtag":"#yoga #malta #wellness",       "likes":"4 129"},
    "ig-book-shelf":     {"handle":"elena.f",   "caption":"stack for the week",              "hashtag":"#booklover #reading",          "likes":"1 072"},
    "ig-flower-market":  {"handle":"elena.f",   "caption":"Saturday finds",                  "hashtag":"#flowers #market #slowmornings", "likes":"2 456"},

    # TikTok
    "tt-dance-clip":       {"handle":"sofia.f",  "caption":"trend attempt no 4 😅",           "hashtag":"#fyp #dance",        "likes":"48K",  "comments":"1.2K", "saves":"3.4K", "music":"trending sound"},
    "tt-cooking-mama":     {"handle":"nanna",    "caption":"stuffat tal-fenek is grandma's",   "hashtag":"#malta #cooking",    "likes":"212K", "comments":"5.1K", "saves":"18K",  "music":"original sound — @nanna"},
    "tt-outfit-mirror":    {"handle":"elena.f",  "caption":"outfit for the day",              "hashtag":"#ootd #fashion",     "likes":"31K",  "comments":"890",  "saves":"2.1K", "music":"trending sound"},
    "tt-street-food-eat":  {"handle":"vincent",  "caption":"first pastizzi review 🥐",         "hashtag":"#malta #foodie",     "likes":"89K",  "comments":"2.4K", "saves":"7.2K", "music":"original sound — @vincent"},
    "tt-travel-walking":   {"handle":"vincent",  "caption":"POV Valletta at golden hour",     "hashtag":"#travel #malta",     "likes":"156K", "comments":"3.2K", "saves":"12K",  "music":"lofi beats"},
    "tt-workout-gym":      {"handle":"liam.f",   "caption":"day 47/90 cut",                   "hashtag":"#gym #fitness",      "likes":"22K",  "comments":"612",  "saves":"1.4K", "music":"phonk mix"},
    "tt-diy-craft":        {"handle":"elena.f",  "caption":"weekend pottery project",         "hashtag":"#pottery #diy",      "likes":"18K",  "comments":"430",  "saves":"890",  "music":"chill acoustic"},
    "tt-pet-cat":          {"handle":"sofia.f",  "caption":"pixel's yarn obsession",          "hashtag":"#cat #catsoftiktok", "likes":"342K", "comments":"6.8K", "saves":"24K",  "music":"cute sound"},
    "tt-morning-routine":  {"handle":"elena.f",  "caption":"6am skincare routine",            "hashtag":"#morning #skincare", "likes":"41K",  "comments":"1.1K", "saves":"5.3K", "music":"soft piano"},
    "tt-cafe-review":      {"handle":"vincent",  "caption":"best flat white in Valletta",    "hashtag":"#coffee #malta",     "likes":"29K",  "comments":"720",  "saves":"2.8K", "music":"cafe jazz"},
    "tt-outdoor-hike":     {"handle":"vincent",  "caption":"view from Dingli cliffs",         "hashtag":"#hike #malta",       "likes":"67K",  "comments":"1.5K", "saves":"4.2K", "music":"acoustic guitar"},

    # YouTube
    "yt-thumbnail-cook":   {"title":"MY NONNA'S secret ragù (never told anyone)", "time":"8:24 / 14:12"},
    "yt-tech-review":      {"title":"The iPhone 16 Pro — 30 days later. Honest.", "time":"12:04 / 18:47"},
    "yt-travel-vlog":      {"title":"Sicily on a budget — 5 days, 350 euros",     "time":"6:18 / 22:11"},
    "yt-fitness-outdoor":  {"title":"BEACH WORKOUT that changed my abs (no gym)", "time":"3:42 / 11:29"},
    "yt-diy-workshop":     {"title":"I built a bookshelf without power tools",    "time":"9:56 / 24:03"},
}

def chrome_overlay(brand, meta=None):
    """Return HTML for the platform UI chrome, with per-image metadata baked in
    (handle, caption, likes, etc). In the real app these come from the catalog's
    metadata fields (xmp_caption + new socials columns) — here we hand-curate
    per image so the mock matches what a shipped app would show."""
    if meta is None:
        meta = SOCIAL_META_DEFAULT.get(brand, {})
    if brand == "instagram":
        handle  = meta.get("handle", "vincent.f")
        caption = meta.get("caption", "")
        hashtag = meta.get("hashtag", "")
        likes   = meta.get("likes", "0")
        return f'''
          <div class="chrome ig-chrome">
            <div class="ig-top">
              <div class="ig-avatar">{handle[0].upper()}</div>
              <div class="ig-handle">{handle}</div>
              <div class="ig-dots">···</div>
            </div>
            <div class="ig-bot">
              <div class="ig-icons">
                <svg viewBox="0 0 24 24" width="26" height="26" fill="none" stroke="#fff" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M20.84 4.61a5.5 5.5 0 0 0-7.78 0L12 5.67l-1.06-1.06a5.5 5.5 0 0 0-7.78 7.78l1.06 1.06L12 21.23l7.78-7.78 1.06-1.06a5.5 5.5 0 0 0 0-7.78z"/></svg>
                <svg viewBox="0 0 24 24" width="26" height="26" fill="none" stroke="#fff" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M21 12a9 9 0 1 1-3.7-7.3L21 3v7"/></svg>
                <svg viewBox="0 0 24 24" width="26" height="26" fill="none" stroke="#fff" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><line x1="22" y1="2" x2="11" y2="13"/><polygon points="22 2 15 22 11 13 2 9 22 2"/></svg>
                <svg viewBox="0 0 24 24" width="26" height="26" fill="none" stroke="#fff" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" style="margin-left:auto"><path d="M19 21l-7-5-7 5V5a2 2 0 0 1 2-2h10a2 2 0 0 1 2 2z"/></svg>
              </div>
              <div class="ig-likes">{likes} likes</div>
              <div class="ig-caption"><b>{handle}</b> {escape(caption)} <span>{escape(hashtag)}</span></div>
            </div>
          </div>'''
    if brand == "tiktok":
        handle   = meta.get("handle", "vincent")
        caption  = meta.get("caption", "")
        hashtag  = meta.get("hashtag", "")
        likes    = meta.get("likes", "128K")
        comments = meta.get("comments", "2.8K")
        saves    = meta.get("saves", "12K")
        music    = meta.get("music", "original sound")
        return f'''
          <div class="chrome tt-chrome">
            <div class="tt-side">
              <div class="tt-avatar-wrap">
                <div class="tt-avatar">{handle[0].upper()}</div>
                <div class="tt-follow">+</div>
              </div>
              <div class="tt-icon"><svg viewBox="0 0 24 24" fill="#fff"><path d="M20.84 4.61a5.5 5.5 0 0 0-7.78 0L12 5.67l-1.06-1.06a5.5 5.5 0 0 0-7.78 7.78l1.06 1.06L12 21.23l7.78-7.78 1.06-1.06a5.5 5.5 0 0 0 0-7.78z"/></svg><span>{likes}</span></div>
              <div class="tt-icon"><svg viewBox="0 0 24 24" fill="none" stroke="#fff" stroke-width="2.2" stroke-linejoin="round"><path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/></svg><span>{comments}</span></div>
              <div class="tt-icon"><svg viewBox="0 0 24 24" fill="#fff"><path d="M6 3a1 1 0 0 0-1 1v17l7-5 7 5V4a1 1 0 0 0-1-1H6z"/></svg><span>{saves}</span></div>
              <div class="tt-icon"><svg viewBox="0 0 24 24" fill="none" stroke="#fff" stroke-width="2" stroke-linejoin="round"><path d="M4 12c0 4.4 3.6 8 8 8s8-3.6 8-8-3.6-8-8-8"/><path d="M12 8v4l3 2"/></svg><span>Share</span></div>
              <div class="tt-disc"><svg viewBox="0 0 24 24" fill="#fff"><circle cx="12" cy="12" r="11" fill="#333"/><circle cx="12" cy="12" r="3" fill="#fff"/><path d="M12 3a9 9 0 0 1 9 9" stroke="#fff" stroke-width="1" fill="none"/></svg></div>
            </div>
            <div class="tt-bot">
              <div class="tt-handle">@{handle}</div>
              <div class="tt-cap">{escape(caption)} <span>{escape(hashtag)}</span></div>
              <div class="tt-sound">♪ {escape(music)}</div>
            </div>
          </div>'''
    if brand == "youtube":
        title = meta.get("title", "Untitled video")
        time  = meta.get("time",  "0:00 / 0:00")
        return f'''
          <div class="chrome yt-chrome">
            <div class="yt-title">{escape(title)}</div>
            <div class="yt-bar">
              <div class="yt-prog"><div class="yt-prog-fill"></div></div>
              <div class="yt-row">
                <span class="yt-time">{escape(time)}</span>
                <span class="yt-badges">
                  <span class="yt-badge">CC</span>
                  <span class="yt-badge">HD</span>
                  <span class="yt-badge">⚙</span>
                </span>
              </div>
            </div>
          </div>'''
    return ""

def _guess_place(name):
    n = name.lower()
    for k, v in PLACE_HINTS.items():
        if k in n:
            return v
    return None

def _guess_event(name):
    n = name.lower()
    for k, v in EVENT_HINTS.items():
        if k in n:
            return v
    return None

def _pretty(name):
    return name.replace("-", " ").replace("_", " ").capitalize()

def write_app_manifest(sections_data):
    """Emit mock/gallery-manifest.js + preview mirror. Every real image on
    disk becomes a catalog entry the app can drop straight into its rows —
    place/event/camera/kind/starred inferred from filename + section so the
    tile chrome matches what a shipped app would show. The IDs are stable
    so re-runs don't churn the app state."""
    from datetime import datetime, timedelta
    now = datetime(2026, 9, 13, 18, 0, 0)
    items = []
    idx = 0
    for section_id, entries in sections_data.items():
        for name, url in entries:
            # url is like /preview-x8f2r7/gallery/hero/x.png — the app is
            # served at /preview-x8f2r7/, so relative "gallery/..." works.
            rel = url.split("/preview-x8f2r7/", 1)[-1]
            place  = _guess_place(name)
            event  = _guess_event(name)
            camera = CAMERA_BY_SECTION.get(section_id, "iPhone 15 Pro")
            kind = "video" if section_id == "video-still" else "photo"
            aspect = ASPECTS_BY_SECTION.get(section_id, "landscape")
            starred = section_id in ("starred", "hero", "highlights") and (idx % 4 == 0)
            # Social sections carry the platform chrome metadata
            social = None
            if section_id.startswith("social-"):
                brand = section_id.split("-", 1)[1]
                meta = SOCIAL_META.get(name, SOCIAL_META_DEFAULT.get(brand, {}))
                social = {"brand": brand, **meta}
            taken = now - timedelta(days=idx * 4 + (idx % 7))
            items.append({
                "id":         f"real-{section_id}-{name}",
                "thumb":      rel,
                "section":    section_id,
                "aspect":     aspect,
                "kind":       kind,
                "duration_s": (12 + (idx * 3) % 45) if kind == "video" else None,
                "taken_at":   taken.isoformat() + "Z",
                "camera":     camera,
                "drive":      "iPhone-15-Pro" if idx % 3 == 0 else ("Nikon-Card" if idx % 3 == 1 else "NAS · Family"),
                "place":      place,
                "event":      event,
                "starred":    starred,
                "raw":        section_id == "raw-technical",
                "featured":   section_id == "hero" and idx < 6,
                "pretty":     _pretty(name),
                "social":     social,
            })
            idx += 1
    payload = "window.SNAPIT_GALLERY = " + json.dumps(items, indent=2) + ";\n"
    (MOCK_ROOT / "gallery-manifest.js").write_text(payload)
    (SERVE / "gallery-manifest.js").write_text(payload)
    print(f"[regen] wrote gallery-manifest.js · {len(items)} real items")

def render():
    sections_data = scan_sections()
    total_count = sum(len(v) for v in sections_data.values())
    write_app_manifest(sections_data)
    section_html = []
    for section_id, label, desc in SECTIONS:
        items = sections_data.get(section_id, [])
        if not items:
            continue
        tiles = "\n".join(render_tile(section_id, name, url) for name, url in items)
        grid_cls = ASPECTS_BY_SECTION.get(section_id, "landscape")
        section_html.append(f'''
<section id="{section_id}">
  <div class="sec-head">
    <span class="tag">{len(items)}</span>
    <h2>{label}</h2>
    <div class="desc">{desc}</div>
  </div>
  <div class="grid {grid_cls}">
    {tiles}
  </div>
</section>''')
    nav_html = "\n".join(
        f'<a href="#{s}">{label}</a>' for s, label, _ in SECTIONS if sections_data.get(s)
    )
    html = TEMPLATE.replace("{{TOTAL}}", str(total_count)) \
                    .replace("{{SECTIONS}}", "\n".join(section_html)) \
                    .replace("{{NAV}}", nav_html)
    (SERVE / "showcase.html").write_text(html)
    print(f"[regen] wrote showcase.html · {total_count} images across {sum(1 for s,_,_ in SECTIONS if sections_data.get(s))} sections")

TEMPLATE = '''<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>SnapIT — showcase gallery · {{TOTAL}} shots</title>
<style>
@font-face{font-family:'Hanken';src:url('/fonts/hanken.woff2') format('woff2');font-weight:100 900;font-display:swap}
@font-face{font-family:'Instrument';src:url('/fonts/instrument-italic.woff2') format('woff2');font-style:italic;font-display:swap}
:root{
  --bg:#0a0b0d; --bg-2:#141419; --bg-3:#1c1c22;
  --ink:#f4f5f8; --dim:#a0a3ac; --line:rgba(255,255,255,.08);
  --core:#4a94ff; --core-deep:#1f6ff2; --gold:#f5c15d; --ok:#3ecf8e;
}
*{margin:0;padding:0;box-sizing:border-box}
html,body{background:var(--bg);color:var(--ink);
  font-family:'Hanken',-apple-system,'SF Pro Text','Inter',system-ui,sans-serif;
  font-size:15px;line-height:1.55;-webkit-font-smoothing:antialiased}
em.i{font-family:'Instrument',serif;font-style:italic;color:var(--core)}
a{color:var(--core);text-decoration:none}

.top{padding:52px 4vw 24px;max-width:1600px;margin:0 auto;text-align:center}
.top .k{color:var(--core);font-size:11px;letter-spacing:.32em;text-transform:uppercase;font-weight:700;margin-bottom:14px}
.top h1{font-size:clamp(42px,5vw,72px);font-weight:600;letter-spacing:-.028em;line-height:1.02}
.top h1 em{color:var(--core);font-family:'Instrument',serif;font-style:italic;font-weight:400}
.top .sub{color:var(--dim);font-size:16px;max-width:800px;margin:20px auto 0;line-height:1.65}
.top .sub b{color:var(--ink);font-weight:600}

.nav{position:sticky;top:0;z-index:10;padding:12px 4vw;background:rgba(10,11,13,.9);
  backdrop-filter:blur(14px);border-bottom:1px solid var(--line);
  display:flex;justify-content:center;gap:6px;flex-wrap:wrap;margin:36px 0 0}
.nav a{color:var(--dim);font-size:12px;letter-spacing:.06em;text-transform:uppercase;font-weight:600;
  padding:6px 12px;border-radius:99px;transition:all .18s}
.nav a:hover{color:var(--ink);background:var(--bg-2)}

section{max-width:1600px;margin:0 auto;padding:44px 4vw}
.sec-head{display:flex;align-items:baseline;gap:14px;margin-bottom:22px;padding-bottom:12px;border-bottom:1px solid var(--line)}
.sec-head h2{font-size:30px;font-weight:600;letter-spacing:-.02em}
.sec-head .tag{color:var(--core);font-size:26px;font-weight:700;letter-spacing:-.03em;font-family:'Hanken';min-width:36px}
.sec-head .desc{color:var(--dim);font-size:13.5px;margin-left:auto;text-align:right;max-width:560px}

.grid{display:grid;gap:14px}
.grid.landscape{grid-template-columns:repeat(auto-fill,minmax(340px,1fr))}
.grid.portrait{grid-template-columns:repeat(auto-fill,minmax(220px,1fr))}
.grid.square{grid-template-columns:repeat(auto-fill,minmax(260px,1fr))}
.grid.story{grid-template-columns:repeat(auto-fill,minmax(180px,1fr))}
/* Chrome-overlay toggle affordance */
.chrome-toggle-wrap{position:sticky;top:56px;z-index:9;background:rgba(10,11,13,.9);
  backdrop-filter:blur(14px);padding:14px 4vw;display:flex;justify-content:center;align-items:center;gap:12px;
  border-bottom:1px solid var(--line);font-size:12.5px;color:var(--dim);letter-spacing:.02em}
.chrome-toggle-wrap .lbl{color:var(--ink)}
.chrome-toggle{position:relative;width:42px;height:22px;background:rgba(255,255,255,.14);border-radius:99px;
  cursor:pointer;transition:background .22s;flex-shrink:0}
.chrome-toggle::after{content:"";position:absolute;top:2px;left:2px;width:18px;height:18px;background:#fff;
  border-radius:50%;transition:transform .28s cubic-bezier(.2,.7,.2,1);box-shadow:0 1px 3px rgba(0,0,0,.4)}
.chrome-toggle.on{background:var(--core);box-shadow:0 0 0 3px rgba(74,148,255,.18)}
.chrome-toggle.on::after{transform:translateX(20px)}

.tile{background:var(--bg-2);border:1px solid var(--line);border-radius:12px;overflow:hidden;position:relative;
  transition:transform .24s cubic-bezier(.2,.7,.2,1),border-color .24s,box-shadow .24s;cursor:zoom-in}
.tile:hover{transform:translateY(-3px);border-color:rgba(255,255,255,.2);box-shadow:0 20px 40px -20px rgba(0,0,0,.7)}
.tile img{width:100%;display:block;aspect-ratio:16/9;object-fit:cover}
.tile.portrait img{aspect-ratio:896/1152}
.tile.square img{aspect-ratio:1/1}
.tile.story img{aspect-ratio:720/1280}
.tile-cap{padding:8px 12px;background:linear-gradient(180deg,transparent,rgba(0,0,0,.72));
  position:absolute;left:0;right:0;bottom:0;opacity:0;transition:opacity .18s}
.tile:hover .tile-cap{opacity:1}
.tile-cap .name{font-size:11.5px;color:#fff;font-weight:600;text-transform:capitalize}

/* PLATFORM CHROME OVERLAYS */
.chrome{position:absolute;inset:0;pointer-events:none;opacity:0;transition:opacity .25s ease}
body.chrome-on .chrome{opacity:1}
.tile.social .tile-cap{display:none} /* handled by chrome overlay */
/* Instagram */
.ig-chrome{display:flex;flex-direction:column}
.ig-top{display:flex;align-items:center;gap:10px;padding:10px 12px;background:linear-gradient(180deg,rgba(0,0,0,.55),transparent)}
.ig-avatar{width:26px;height:26px;border-radius:50%;background:linear-gradient(135deg,#f58529,#dd2a7b,#8134af);display:flex;align-items:center;justify-content:center;color:#fff;font-weight:700;font-size:12px}
.ig-handle{color:#fff;font-size:12px;font-weight:600}
.ig-dots{margin-left:auto;color:#fff;font-size:16px}
.ig-bot{margin-top:auto;padding:12px 14px;background:linear-gradient(0deg,rgba(0,0,0,.85),transparent);color:#fff}
.ig-icons{display:flex;gap:14px;align-items:center;margin-bottom:8px}
.ig-likes{font-size:12px;font-weight:700}
.ig-caption{font-size:11.5px;margin-top:2px;opacity:.95}
.ig-caption span{color:#7cd0ff}
/* TikTok — sized to real-app proportions: rail hugs the right, starts about
   midway down the video (not bottom-glued), icons ~9% of tile width, video
   is always the hero. */
.tt-chrome{display:flex}
.tt-side{margin-left:auto;width:20%;max-width:36px;padding:0 3px 6px;display:flex;flex-direction:column;
  justify-content:flex-end;gap:7px;align-items:center;color:#fff}
.tt-avatar-wrap{position:relative;margin-bottom:8px}
.tt-avatar{width:20px;height:20px;border-radius:50%;background:linear-gradient(135deg,#ff0050,#00f2ea);
  display:flex;align-items:center;justify-content:center;color:#fff;font-weight:700;font-size:9px;
  border:1.5px solid #fff}
.tt-follow{position:absolute;bottom:-6px;left:50%;transform:translateX(-50%);width:11px;height:11px;
  border-radius:50%;background:#fe2c55;color:#fff;display:flex;align-items:center;justify-content:center;
  font-size:9px;font-weight:800;line-height:1;border:1px solid rgba(0,0,0,.14)}
.tt-icon{display:flex;flex-direction:column;align-items:center;gap:0;font-size:8px;font-weight:600;
  text-shadow:0 1px 2px rgba(0,0,0,.7);line-height:1.2}
.tt-icon svg{width:17px;height:17px;filter:drop-shadow(0 1px 2px rgba(0,0,0,.7))}
.tt-disc{margin-top:2px}
.tt-disc svg{width:20px;height:20px;filter:drop-shadow(0 1px 2px rgba(0,0,0,.7));animation:tt-spin 4s linear infinite}
@keyframes tt-spin{to{transform:rotate(360deg)}}
.tt-bot{position:absolute;bottom:8px;left:8px;right:36px;color:#fff}
.tt-handle{font-weight:800;font-size:10.5px;text-shadow:0 1px 2px rgba(0,0,0,.8);letter-spacing:.01em}
.tt-cap{font-size:9px;line-height:1.35;margin-top:2px;text-shadow:0 1px 2px rgba(0,0,0,.75);
  display:-webkit-box;-webkit-line-clamp:2;-webkit-box-orient:vertical;overflow:hidden}
.tt-cap span{color:#dceaff}
.tt-sound{font-size:8.5px;margin-top:4px;padding:2px 5px;background:rgba(0,0,0,.35);border-radius:99px;
  display:inline-block;backdrop-filter:blur(3px);text-shadow:0 1px 1px rgba(0,0,0,.6)}
/* YouTube */
.yt-chrome{}
.yt-title{position:absolute;top:12px;left:14px;right:14px;color:#fff;font-size:13px;font-weight:600;line-height:1.35;text-shadow:0 1px 3px rgba(0,0,0,.75);opacity:.95}
.yt-bar{position:absolute;left:0;right:0;bottom:0;padding:0 12px 8px;color:#fff}
.yt-prog{height:3px;background:rgba(255,255,255,.28);border-radius:99px;margin-bottom:6px;overflow:hidden}
.yt-prog-fill{height:100%;width:33%;background:#ff0000}
.yt-row{display:flex;justify-content:space-between;align-items:center;font-size:11px}
.yt-time{font-weight:600}
.yt-badges{display:flex;gap:6px}
.yt-badge{padding:1px 5px;background:rgba(0,0,0,.5);border-radius:3px;font-size:10px}

/* Lightbox */
.lb{position:fixed;inset:0;background:rgba(0,0,0,.94);display:flex;align-items:center;justify-content:center;
  padding:40px;z-index:80;opacity:0;pointer-events:none;transition:opacity .3s}
.lb.on{opacity:1;pointer-events:all}
.lb img{max-width:100%;max-height:100%;border-radius:8px;box-shadow:0 30px 80px -20px rgba(0,0,0,.9)}
.lb-close{position:absolute;top:20px;right:24px;color:#fff;font-size:26px;background:rgba(0,0,0,.6);
  width:40px;height:40px;border-radius:50%;display:flex;align-items:center;justify-content:center;cursor:pointer}

.foot{padding:60px 4vw 100px;text-align:center;color:var(--dim);font-size:13px;max-width:900px;margin:0 auto}
.foot b{color:var(--ink)}
.foot em{color:var(--core);font-family:'Instrument',serif;font-style:italic}
.foot .cta{margin-top:26px;display:inline-block;padding:12px 32px;background:var(--core);color:#fff;
  border-radius:99px;font-weight:600;letter-spacing:.02em;font-size:13.5px;transition:all .18s}
.foot .cta:hover{background:var(--core-deep);transform:translateY(-1px)}
</style>
</head>
<body>

<header class="top">
  <div class="k">Showcase gallery · {{TOTAL}} shots</div>
  <h1>Every event, every drive, <em>one library.</em></h1>
  <p class="sub">
    Real examples of what a SnapIT library actually looks like — hero events, highlights, portraits, birthdays, weddings, home renovations, travel, videos, socials (with and without platform chrome), starred details, places, still-life, kids, RAW technical. <b>Everything you'd see on your own machine, generated locally, filed by story.</b>
  </p>
</header>

<nav class="nav">
  {{NAV}}
</nav>

<div class="chrome-toggle-wrap">
  <span>Show platform chrome on socials</span>
  <div class="chrome-toggle" id="chromeToggle" onclick="toggleChrome()"></div>
  <span class="lbl" id="chromeLabel">Off · showing clean media</span>
</div>

{{SECTIONS}}

<div class="foot">
  <p>The full showcase. If you save a TikTok via our extension you get the <b>clean video</b> — no watermark, no UI overlays, just the original media with the caption + author + likes stored as fields you can search. If you took your own screenshot, we handle that too — the platform chrome is baked into the pixels because that's what your OS captured. Same library. Same rows. Nothing left behind.</p>
  <a class="cta" href="/preview-x8f2r7/">← Back to the app mock</a>
</div>

<div class="lb" id="lb" onclick="lbClose()">
  <img id="lbImg" src="" alt="">
  <div class="lb-close">×</div>
</div>

<script>
function lb(el) {
    const img = el.querySelector('img');
    document.getElementById('lbImg').src = img.src;
    document.getElementById('lb').classList.add('on');
}
function lbClose() { document.getElementById('lb').classList.remove('on'); }
document.addEventListener('keydown', e => { if (e.key === 'Escape') lbClose(); });

/* Chrome-overlay toggle — same setting shape as the per-profile toggle in the real app. */
function applyChrome(on) {
    document.body.classList.toggle('chrome-on', on);
    document.getElementById('chromeToggle').classList.toggle('on', on);
    document.getElementById('chromeLabel').textContent = on
        ? 'On · showing screenshot with platform overlay'
        : 'Off · showing clean media (extension / takeout import)';
    try { localStorage.setItem('snapit_chrome_on', on ? '1' : '0'); } catch (_) {}
}
function toggleChrome() {
    const on = !document.body.classList.contains('chrome-on');
    applyChrome(on);
}
try {
    const stored = localStorage.getItem('snapit_chrome_on');
    if (stored === '1') applyChrome(true);
    else applyChrome(false);
} catch (_) { applyChrome(false); }
</script>

</body>
</html>
'''

if __name__ == "__main__":
    mirror_gallery()
    render()
