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

// Build 40 photo/video items using the pre-rendered mock thumbs.
const items = [];
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

// Build rows.
function rowByYear() {
    const byYear = {};
    for (const it of items) {
        const y = new Date(it.taken_at).getUTCFullYear();
        (byYear[y] ??= []).push(it);
    }
    return Object.entries(byYear)
        .sort((a, b) => Number(b[0]) - Number(a[0]))
        .map(([y, list]) => ({
            id: `year-${y}`,
            title: y,
            subtitle: `${list.length} · ${list.filter((i) => i.kind === 'video').length} videos`,
            items: list,
        }));
}

function rowByEvent() {
    // Random distribution across event names for the mock.
    const shuffled = [...items].sort(() => Math.random() - 0.5);
    const rows = [];
    let idx = 0;
    for (const name of EVENT_NAMES) {
        const size = 3 + Math.floor(Math.random() * 4);
        const chunk = shuffled.slice(idx, idx + size);
        if (chunk.length === 0) break;
        rows.push({
            id: `event-${name.toLowerCase().replace(/[^a-z0-9]+/g, '-')}`,
            title: name,
            subtitle: `${chunk.length} · ${chunk[0].place ?? ''}`,
            items: chunk,
        });
        idx += size;
    }
    return rows;
}

function rowByPeople() {
    const map = {};
    for (const it of items) {
        for (const p of it.people) {
            (map[p] ??= []).push(it);
        }
    }
    return Object.entries(map)
        .sort((a, b) => b[1].length - a[1].length)
        .map(([p, list]) => ({
            id: `people-${p.toLowerCase()}`,
            title: p,
            subtitle: `${list.length} photos & videos`,
            items: list,
        }));
}

function rowByPlace() {
    const map = {};
    for (const it of items) {
        if (!it.place) continue;
        (map[it.place] ??= []).push(it);
    }
    return Object.entries(map)
        .sort((a, b) => b[1].length - a[1].length)
        .map(([p, list]) => ({
            id: `place-${p.toLowerCase()}`,
            title: p,
            subtitle: `${list.length} photos`,
            items: list,
        }));
}

function rowRecent() {
    const sorted = [...items].sort(
        (a, b) => new Date(b.taken_at) - new Date(a.taken_at)
    );
    return {
        id: 'recent',
        title: 'Recently added',
        subtitle: 'Last 30 days · imported today',
        items: sorted.slice(0, 12),
    };
}

function rowStarred() {
    const starred = items.filter((i) => i.starred);
    return {
        id: 'starred',
        title: 'Starred',
        subtitle: `${starred.length} favourites`,
        items: starred,
    };
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
        recent: rowRecent(),
        starred: rowStarred(),
        year: rowByYear(),
        event: rowByEvent(),
        people: rowByPeople(),
        place: rowByPlace(),
    },
};
