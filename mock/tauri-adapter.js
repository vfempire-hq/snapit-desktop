// tauri-adapter.js — bridges the mock UI to the real Tauri backend when
// running inside the desktop app. In the browser preview this file is a
// silent no-op, so the CF-served demo experience is unchanged.
//
// Responsibilities:
//   1. Detect Tauri and expose window.SnapitTauri for the mock to call.
//   2. If a library is already open (auto-rehydrated from prefs on boot),
//      pull the recent-photos list + swap window.SNAPIT_GALLERY with real
//      content, then re-render the current view.
//   3. Expose openLibrary() / pickLibrary() so the "Add source" flow can
//      actually mount a real folder.
//   4. Wire the file-watcher's 'snapit://scan/progress' events to a small
//      status toast so the user sees indexing happening.
//
// Everything happens after DOM ready so the mock's own boot animations run
// unimpeded — this adapter is additive.

(function () {
    const T = window.__TAURI_INTERNALS__ || window.__TAURI__?.core;
    if (!T || typeof T.invoke !== 'function') {
        // Web preview — nothing to do.
        window.SnapitTauri = null;
        return;
    }

    const invoke = T.invoke.bind(T);
    // Tauri v2 with `withGlobalTauri: true` exposes convertFileSrc at
    // __TAURI__.core.convertFileSrc; fall back to the internals namespace.
    const convertFileSrc = (window.__TAURI__?.core?.convertFileSrc)
        || (window.__TAURI_INTERNALS__?.convertFileSrc)
        || ((p) => 'file://' + p);
    // Dialog plugin — for the folder picker.
    const dialog = window.__TAURI__?.dialog;

    // ---- helpers ------------------------------------------------------

    function isRawExt(path) {
        return /\.(nef|cr2|cr3|arw|dng|raf|rw2|orf|pef|srw)$/i.test(path);
    }

    function prettyFromPath(p) {
        // Take the file stem, strip camera-format prefixes, replace
        // separators with spaces, capitalise.
        const stem = (p.split(/[\\/]/).pop() || p).replace(/\.[^.]+$/, '');
        return stem.replace(/^(IMG|DSC|DSCN|MVI|PXL|VID)[-_]?0*/i, '')
                   .replace(/[-_]+/g, ' ')
                   .trim() || stem;
    }

    /**
     * Turn a Rust HydratedRow into the shape the mock's data.js + tile
     * renderers already understand (matches gallery-manifest.js output).
     */
    function rowToGalleryItem(row) {
        const isPortrait = row.height > row.width * 1.1;
        const isSquare   = Math.abs(row.width - row.height) < Math.max(row.width, row.height) * 0.1;
        return {
            id:         row.id,
            thumb:      convertFileSrc(row.thumb_path),
            section:    (row.kind === 'video') ? 'video-still'
                       : (row.xmp_rating || 0) >= 4 ? 'starred'
                       : isPortrait ? 'portrait'
                       : 'hero',
            aspect:     isPortrait ? 'portrait' : isSquare ? 'square' : 'landscape',
            kind:       row.kind,
            duration_s: row.duration_s,
            taken_at:   row.taken_at,
            camera:     row.camera || 'Unknown camera',
            // R·02b will add a proper 'source' column to photos and let
            // us render 'iPhone-15-Pro' / 'NAS · Family' etc. For MVP
            // every real photo comes from 'This device' since that's the
            // only source the desktop indexer supports today.
            drive:      'This device',
            // Places require a GPS reverse-geocode pass (bundled gazetteer
            // planned for R·02b). Null keeps the UI clean until then.
            place:      null,
            starred:    (row.xmp_rating || 0) >= 4,
            raw:        isRawExt(row.path),
            featured:   false,
            pretty:     row.caption || prettyFromPath(row.path),
            social:     null,
        };
    }

    /**
     * Fetch the current library's recent photos, swap SNAPIT_GALLERY with
     * real content, and force the mock to re-render.
     */
    async function refreshFromCatalog() {
        try {
            const state = await invoke('catalog_state');
            if (!state.library_path || state.photo_count === 0) {
                // No library open OR library is empty — leave the demo
                // gallery in place so the app doesn't look broken.
                return { ok: false, reason: 'empty' };
            }
            const rows = await invoke('catalog_recent_hydrated', {
                limit: 200,
                minRating: 0,
            });
            if (!rows || !rows.length) return { ok: false, reason: 'no_rows' };
            window.SNAPIT_GALLERY = rows.map(rowToGalleryItem);
            // The mock's data.js already fires once at load with the
            // legacy demo manifest. To pick up the new items we need to
            // re-run its build. Dispatching the event triggers a helper
            // wired below the adapter's IIFE.
            window.dispatchEvent(new CustomEvent('snapit:gallery-updated', {
                detail: { count: rows.length, source: 'catalog' },
            }));
            return { ok: true, count: rows.length };
        } catch (e) {
            console.warn('[tauri-adapter] catalog refresh failed:', e);
            return { ok: false, reason: String(e) };
        }
    }

    /**
     * Open a folder picker → mount it as the library → scan → refresh.
     * Called by the "Add source" flow when the user picks
     * "This device / choose a folder".
     */
    async function pickLibrary() {
        if (!dialog?.open) throw new Error('dialog plugin unavailable');
        const selected = await dialog.open({
            directory: true,
            multiple: false,
            title: 'Choose a folder to add to SnapIT',
        });
        if (!selected) return null;
        const path = Array.isArray(selected) ? selected[0] : selected;
        return openLibrary(path);
    }

    async function openLibrary(path) {
        await invoke('catalog_open', { path });
        // Kick off a scan; progress events fire as photos land. The
        // library_scan promise resolves once the walk finishes.
        const count = await invoke('library_scan', { path });
        await refreshFromCatalog();
        return { path, count };
    }

    // ---- expose to the mock -------------------------------------------

    window.SnapitTauri = {
        refreshFromCatalog,
        openLibrary,
        pickLibrary,
        catalogState: () => invoke('catalog_state'),
    };
    window.SNAPIT_IS_TAURI = true;

    // ---- auto-boot: try to pull real content once the mock finishes ---
    // The mock renders home ~2s after DOMContentLoaded (splash + picker
    // + first paint). Wait a beat, then try to swap in real photos. If
    // no library is open, this is a no-op and the demo gallery stays.
    window.addEventListener('DOMContentLoaded', () => {
        setTimeout(refreshFromCatalog, 1200);
    });
})();

// ---- gallery re-render hook (runs in both Tauri and web) ------------
// Every time SNAPIT_GALLERY changes we want the mock to redraw its rows
// so the tiles reflect the new dataset. Data.js exposes MOCK_LIBRARY as
// window.MOCK_LIBRARY; renderHome() reads from it.
window.addEventListener('snapit:gallery-updated', () => {
    // Rebuild MOCK_LIBRARY from the new SNAPIT_GALLERY, then repaint.
    // rebuildLibraryData is defined in data.js's IIFE wrapper.
    if (typeof window.rebuildLibraryData === 'function') {
        window.rebuildLibraryData();
    }
    if (typeof window.rerenderCurrentView === 'function') {
        window.rerenderCurrentView();
    }
});
