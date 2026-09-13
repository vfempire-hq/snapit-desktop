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

# Aspect classification by filename convention — set from prompt files
ASPECTS_BY_SECTION = {
    "hero":            "landscape",  # 1344x768
    "highlights":      "landscape",
    "video-still":     "landscape",
    "place":           "landscape",
    "kids-row":        "landscape",
    "raw-technical":   "landscape",
    "social-youtube":  "landscape",
    "portrait":        "portrait",   # 896x1152
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
    the global 'Show platform chrome' toggle."""
    aspect = ASPECTS_BY_SECTION.get(section_id, "landscape")
    if section_id.startswith("social-"):
        brand = section_id.split("-", 1)[1]
        return f'''
        <div class="tile {aspect} social {brand}" onclick="lb(this)">
          <img src="{url}" alt="" loading="lazy">
          {chrome_overlay(brand)}
          <div class="tile-cap"><div class="name">{escape(name.replace("-"," "))}</div></div>
        </div>'''
    return f'''
        <div class="tile {aspect}" onclick="lb(this)">
          <img src="{url}" alt="" loading="lazy">
          <div class="tile-cap"><div class="name">{escape(name.replace("-"," "))}</div></div>
        </div>'''

def chrome_overlay(brand):
    """Return HTML for the platform UI chrome that would be baked into a screenshot."""
    if brand == "instagram":
        return '''
          <div class="chrome ig-chrome">
            <div class="ig-top">
              <div class="ig-avatar">V</div>
              <div class="ig-handle">@vincent.f</div>
              <div class="ig-dots">···</div>
            </div>
            <div class="ig-bot">
              <div class="ig-icons">
                <svg viewBox="0 0 24 24" width="26" height="26" fill="none" stroke="#fff" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M20.84 4.61a5.5 5.5 0 0 0-7.78 0L12 5.67l-1.06-1.06a5.5 5.5 0 0 0-7.78 7.78l1.06 1.06L12 21.23l7.78-7.78 1.06-1.06a5.5 5.5 0 0 0 0-7.78z"/></svg>
                <svg viewBox="0 0 24 24" width="26" height="26" fill="none" stroke="#fff" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M21 12a9 9 0 1 1-3.7-7.3L21 3v7"/></svg>
                <svg viewBox="0 0 24 24" width="26" height="26" fill="none" stroke="#fff" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><line x1="22" y1="2" x2="11" y2="13"/><polygon points="22 2 15 22 11 13 2 9 22 2"/></svg>
                <svg viewBox="0 0 24 24" width="26" height="26" fill="none" stroke="#fff" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" style="margin-left:auto"><path d="M19 21l-7-5-7 5V5a2 2 0 0 1 2-2h10a2 2 0 0 1 2 2z"/></svg>
              </div>
              <div class="ig-likes">1 284 likes</div>
              <div class="ig-caption"><b>vincent.f</b> Malta trip · June. <span>#malta #summer26</span></div>
            </div>
          </div>'''
    if brand == "tiktok":
        return '''
          <div class="chrome tt-chrome">
            <div class="tt-side">
              <div class="tt-avatar">V</div>
              <div class="tt-icon"><svg viewBox="0 0 24 24" fill="#fff" width="32" height="32"><path d="M20.84 4.61a5.5 5.5 0 0 0-7.78 0L12 5.67l-1.06-1.06a5.5 5.5 0 0 0-7.78 7.78l1.06 1.06L12 21.23l7.78-7.78 1.06-1.06a5.5 5.5 0 0 0 0-7.78z"/></svg><span>128K</span></div>
              <div class="tt-icon"><svg viewBox="0 0 24 24" fill="#fff" width="30" height="30"><path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/></svg><span>2 890</span></div>
              <div class="tt-icon"><svg viewBox="0 0 24 24" fill="none" stroke="#fff" stroke-width="2" width="30" height="30"><path d="M9 17l6-5-6-5v10z" fill="#fff"/><circle cx="12" cy="12" r="10"/></svg><span>Share</span></div>
            </div>
            <div class="tt-bot">
              <div class="tt-handle">@vincent</div>
              <div class="tt-cap">Sunset over Valletta — first time we tried the DJI 🎥 #malta #summer26 #dji</div>
              <div class="tt-music"><svg viewBox="0 0 24 24" width="12" height="12" fill="#fff"><path d="M9 3v13.5a3.5 3.5 0 1 1-3.5-3.5H6V6h5V3z"/></svg> original sound · @vincent</div>
            </div>
          </div>'''
    if brand == "youtube":
        return '''
          <div class="chrome yt-chrome">
            <div class="yt-title">Renovating our Rostock office — week 8 · full walkthrough in 4K</div>
            <div class="yt-bar">
              <div class="yt-prog"><div class="yt-prog-fill"></div></div>
              <div class="yt-row">
                <span class="yt-time">4:12 / 12:38</span>
                <span class="yt-badges">
                  <span class="yt-badge">CC</span>
                  <span class="yt-badge">HD</span>
                  <span class="yt-badge">⚙</span>
                </span>
              </div>
            </div>
          </div>'''
    return ""

def render():
    sections_data = scan_sections()
    total_count = sum(len(v) for v in sections_data.values())
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
/* TikTok */
.tt-chrome{display:flex}
.tt-side{margin-left:auto;padding:0 10px 28px;display:flex;flex-direction:column;justify-content:flex-end;gap:14px;align-items:center;color:#fff}
.tt-avatar{width:40px;height:40px;border-radius:50%;background:linear-gradient(135deg,#ff0050,#00f2ea);display:flex;align-items:center;justify-content:center;color:#fff;font-weight:700;border:2px solid #fff}
.tt-icon{display:flex;flex-direction:column;align-items:center;gap:2px;font-size:10px;font-weight:600;text-shadow:0 1px 3px rgba(0,0,0,.6)}
.tt-bot{position:absolute;bottom:14px;left:12px;right:80px;color:#fff}
.tt-handle{font-weight:700;font-size:13px;text-shadow:0 1px 3px rgba(0,0,0,.7)}
.tt-cap{font-size:11.5px;line-height:1.4;margin-top:3px;text-shadow:0 1px 3px rgba(0,0,0,.7)}
.tt-music{font-size:10.5px;margin-top:5px;opacity:.9;display:flex;align-items:center;gap:4px;text-shadow:0 1px 3px rgba(0,0,0,.7)}
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
