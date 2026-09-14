// Mock library data for the SnapIT prototype.
// Simulates the shape the real app will hand to the frontend once R·02 lands.

const NOW = new Date('2026-09-13T18:00:00Z');

function pick(arr) {
    return arr[Math.floor(Math.random() * arr.length)];
}

const CAMERAS = [
    'iPhone 15 Pro',
    'Nikon Z8',
    'Sony A7 IV',
    'Fujifilm X-T5',
    'GoPro HERO12',
    'DJI Mavic 3',
    'iPhone 12',
    'Screenshot',
];
const DRIVES = [
    'iPhone-15-Pro',
    'MacBook',
    'Windows-PC',
    'NAS · Family',
    'External-SSD',
    'WhatsApp Media',
];
const PEOPLE = ['Vincent', 'Elena', 'Liam', 'Sofia', 'Marcus', 'Anna'];
const PLACES = ['Malta', 'Rostock', 'Leipzig', 'Berlin', 'Valletta', 'Sicily', 'Zürich'];
const EVENT_NAMES = [
    'Malta trip · June',
    'Berlin office move',
    'Kitchen renovation',
    'Sofia\'s birthday',
    'Property viewing · Sliema',
    'Coffee cupping',
    'Rostock warehouse',
    'Anna\'s wedding',
    'Site walk · Frankfurt',
    'Weekend at the lake',
    'Christmas 2025',
    'New Year in Valletta',
];

// Build the library from either the demo manifest OR — when the Tauri
// adapter has swapped in real content — the actual catalog rows. Wrapped
// in a function so `window.rebuildLibraryData()` can be called any time
// the gallery source changes (e.g. after `library_scan`).
function buildLibrary() {
const items = [];
const REAL = (typeof window !== 'undefined' && Array.isArray(window.SNAPIT_GALLERY))
    ? window.SNAPIT_GALLERY : [];

if (REAL.length >= 10) {
    // Use real images. Fill in missing people randomly so face-grouping
    // demos still work — real app pulls people from ML face-clustering.
    for (const r of REAL) {
        items.push({
            id: r.id,
            thumb: r.thumb,
            kind: r.kind,
            duration_s: r.duration_s,
            taken_at: r.taken_at,
            camera: r.camera,
            drive: r.drive,
            people: Math.random() < 0.6 ? [pick(PEOPLE)] : [],
            place: r.place,
            starred: r.starred,
            raw: r.raw,
            event: r.event,
            featured: r.featured,
            pretty: r.pretty,
            social: r.social || null,
            section: r.section,
        });
    }
} else {
    for (let i = 0; i < 40; i++) {
        const isVideo = i % 7 === 0;
        const daysAgo = Math.floor(i * 8 + Math.random() * 30);
        const taken = new Date(NOW.getTime() - daysAgo * 86400_000);
        items.push({
            id: `mock-${String(i).padStart(3, '0')}`,
            thumb: `assets/thumbs/mock-${String(i).padStart(3, '0')}.jpg`,
            kind: isVideo ? 'video' : 'photo',
            duration_s: isVideo ? Math.floor(20 + Math.random() * 240) : null,
            taken_at: taken.toISOString(),
            camera: pick(CAMERAS),
            drive: pick(DRIVES),
            people: (Math.random() < 0.6 ? [pick(PEOPLE)] : []).concat(
                Math.random() < 0.3 ? [pick(PEOPLE)] : []
            ),
            place: Math.random() < 0.7 ? pick(PLACES) : null,
            starred: Math.random() < 0.18,
            raw: Math.random() < 0.25,
        });
    }
}

    // Row builders read the outer `items` closure — declared here inside
    // buildLibrary() so they see the freshly-built list each rebuild.
    function _byYear() {
        const byYear = {};
        for (const it of items) { const y = new Date(it.taken_at).getUTCFullYear(); (byYear[y] ??= []).push(it); }
        return Object.entries(byYear).sort((a,b)=>Number(b[0])-Number(a[0]))
            .map(([y,list])=>({ id:`year-${y}`, title:y, subtitle:`${list.length} · ${list.filter(i=>i.kind==='video').length} videos`, items:list }));
    }
    function _byEvent() {
        const withEvent = items.filter(i => i.event);
        if (withEvent.length >= 6) {
            const map = {}; for (const it of withEvent) (map[it.event] ??= []).push(it);
            const rest = items.filter(i => !i.event); if (rest.length >= 4) map['Recently added'] = rest;
            return Object.entries(map).map(([name,list])=>({ id:`event-${name.toLowerCase().replace(/[^a-z0-9]+/g,'-')}`, title:name, subtitle:`${list.length} · ${list[0].place ?? ''}`, items:list }));
        }
        const shuffled = [...items].sort(()=>Math.random()-0.5);
        const rows = []; let idx=0;
        for (const name of EVENT_NAMES) {
            const size = 3 + Math.floor(Math.random()*4);
            const chunk = shuffled.slice(idx, idx+size); if (!chunk.length) break;
            rows.push({ id:`event-${name.toLowerCase().replace(/[^a-z0-9]+/g,'-')}`, title:name, subtitle:`${chunk.length} · ${chunk[0].place ?? ''}`, items:chunk });
            idx += size;
        }
        return rows;
    }
    function _byPeople() {
        const map = {}; for (const it of items) for (const p of it.people) (map[p] ??= []).push(it);
        return Object.entries(map).sort((a,b)=>b[1].length-a[1].length)
            .map(([p,list])=>({ id:`people-${p.toLowerCase()}`, title:p, subtitle:`${list.length} photos & videos`, items:list }));
    }
    function _byPlace() {
        const map = {}; for (const it of items) if (it.place) (map[it.place] ??= []).push(it);
        return Object.entries(map).sort((a,b)=>b[1].length-a[1].length)
            .map(([p,list])=>({ id:`place-${p.toLowerCase()}`, title:p, subtitle:`${list.length} photos`, items:list }));
    }
    function _recent() {
        const sorted = [...items].sort((a,b)=>new Date(b.taken_at)-new Date(a.taken_at));
        return { id:'recent', title:'Recently added', subtitle:'Last 30 days · imported today', items:sorted.slice(0,12) };
    }
    function _starred() {
        const s = items.filter(i=>i.starred);
        return { id:'starred', title:'Starred', subtitle:`${s.length} favourites`, items:s };
    }

    window.MOCK_LIBRARY = {
        counts: {
            photos: items.filter((i) => i.kind === 'photo').length,
            videos: items.filter((i) => i.kind === 'video').length,
            events: EVENT_NAMES.length,
            drives: DRIVES.length,
            duplicates_folded: 137,
            gb_freed: 3.4,
        },
        drives: DRIVES.map((name, idx) => ({
            id: `drive-${idx}`,
            name,
            connected: idx < 4,
            used_gb: (12 + Math.random() * 400).toFixed(1),
            photos: 200 + Math.floor(Math.random() * 8000),
        })),
        rows: {
            recent: _recent(),
            starred: _starred(),
            year: _byYear(),
            event: _byEvent(),
            people: _byPeople(),
            place: _byPlace(),
        },
    };
}

// Expose a rebuild trigger for the Tauri adapter (or anyone else who
// wants to swap in a new SNAPIT_GALLERY). Also runs once immediately
// so the mock has data on first paint.
window.rebuildLibraryData = buildLibrary;
buildLibrary();
