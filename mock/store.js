// store.js — SnapIT profile persistence for the mock.
//
// Runs the same shape the Rust profile module runs. When we bundle the app
// as Tauri, this file gets replaced 1:1 with calls to invoke("profile_*")
// against src-tauri/src/profile/mod.rs — every function here has an exact
// Rust twin.
//
// PIN crypto uses PBKDF2-SHA256 with 300 000 iterations (OWASP 2023) via
// Web Crypto, so the mock has a REAL cryptographic PIN check, not a
// pretend-string-compare. Argon2id lands when we bundle (native Rust path).

const STORE_KEY = 'snapit_profiles_v1';
const FAMILY_PACK_MAX = 5;
const PBKDF2_ITERS = 300_000;

// ----- Types (docs only; JS has no static types) -----------------------
// Profile = {
//   id, name, role, initials, gradient, kids: bool, pin_hash: string|null,
//   autoplay_slides: bool, autoplay_previews: bool, face_group_consent: bool,
//   default_save: 'this-profile'|'shared'|'ask',
//   content_restrictions: 'none'|'13+'|'kid-safe',
//   language: string, delete_forbidden: bool, created_at: string
// }
// ProfileStore = { profiles: Profile[], active_profile_id: string|null }

// ---- helpers ---------------------------------------------------------

function _load() {
    try {
        const s = localStorage.getItem(STORE_KEY);
        if (!s) return { profiles: [], active_profile_id: null };
        return JSON.parse(s);
    } catch {
        return { profiles: [], active_profile_id: null };
    }
}

function _save(store) {
    localStorage.setItem(STORE_KEY, JSON.stringify(store));
}

function _uuid() {
    if (crypto.randomUUID) return crypto.randomUUID();
    return 'p_' + Math.random().toString(36).slice(2) + Date.now().toString(36);
}

async function _pbkdf2(pin, salt_b64) {
    const enc = new TextEncoder();
    const salt = _fromB64(salt_b64);
    const keyMaterial = await crypto.subtle.importKey(
        'raw', enc.encode(pin), { name: 'PBKDF2' }, false, ['deriveBits']
    );
    const bits = await crypto.subtle.deriveBits(
        { name: 'PBKDF2', salt, iterations: PBKDF2_ITERS, hash: 'SHA-256' },
        keyMaterial, 256
    );
    return _toB64(new Uint8Array(bits));
}

function _randomSalt() {
    const salt = crypto.getRandomValues(new Uint8Array(16));
    return _toB64(salt);
}

function _toB64(bytes) {
    let s = '';
    for (let i = 0; i < bytes.length; i++) s += String.fromCharCode(bytes[i]);
    return btoa(s);
}
function _fromB64(b64) {
    const s = atob(b64);
    const out = new Uint8Array(s.length);
    for (let i = 0; i < s.length; i++) out[i] = s.charCodeAt(i);
    return out;
}

function _validatePin(pin) {
    if (typeof pin !== 'string') throw new Error('pin_must_be_string');
    const n = pin.length;
    if (n < 4 || n > 8) throw new Error('pin_must_be_4_to_8_digits');
    if (!/^\d+$/.test(pin)) throw new Error('pin_must_be_digits');
}

// Public snapshot the UI receives — hides pin_hash.
function _view(p) {
    return {
        id: p.id, name: p.name, role: p.role, initials: p.initials,
        gradient: p.gradient, kids: !!p.kids, locked: !!p.pin_hash,
        autoplay_slides: !!p.autoplay_slides,
        autoplay_previews: !!p.autoplay_previews,
        face_group_consent: !!p.face_group_consent,
        default_save: p.default_save, content_restrictions: p.content_restrictions,
        language: p.language, delete_forbidden: !!p.delete_forbidden,
        created_at: p.created_at,
    };
}

// ---- public API ------------------------------------------------------

export const ProfileStore = {
    /** Ensure at least one Owner profile exists. */
    async bootstrap() {
        const store = _load();
        if (store.profiles.length > 0) return;
        // Seed the family used in the mock so first-run has something to click.
        const seeds = [
            { name: 'Vincent', role: 'owner',  initials: 'V', gradient: 'gradient-v', kids: false, delete_forbidden: true },
            { name: 'Elena',   role: 'family', initials: 'E', gradient: 'gradient-e', kids: false },
            { name: 'Liam',    role: 'family', initials: 'L', gradient: 'gradient-l', kids: false },
            { name: 'Kids',    role: 'kid',    initials: 'K', gradient: 'gradient-k', kids: true,
              content_restrictions: '13+', autoplay_slides: false, autoplay_previews: false, default_save: 'this-profile' },
        ];
        for (const s of seeds) {
            store.profiles.push({
                id: _uuid(),
                name: s.name, role: s.role, initials: s.initials, gradient: s.gradient,
                kids: !!s.kids,
                pin_hash: null,
                autoplay_slides: s.autoplay_slides ?? true,
                autoplay_previews: s.autoplay_previews ?? true,
                face_group_consent: false,
                default_save: s.default_save ?? 'this-profile',
                content_restrictions: s.content_restrictions ?? 'none',
                language: 'en-GB',
                delete_forbidden: !!s.delete_forbidden,
                created_at: new Date().toISOString(),
            });
        }
        store.active_profile_id = store.profiles[0].id;
        _save(store);
    },

    async list() {
        return _load().profiles.map(_view);
    },

    async activeId() {
        return _load().active_profile_id;
    },

    async setActive(profileId, pin /* optional; required if profile is locked */) {
        const store = _load();
        const p = store.profiles.find(x => x.id === profileId);
        if (!p) throw new Error('profile_not_found');
        if (p.pin_hash) {
            const ok = await this.verifyPin(profileId, pin || '');
            if (!ok) throw new Error('pin_mismatch');
        }
        store.active_profile_id = profileId;
        _save(store);
        return _view(p);
    },

    async create({ name, initials, gradient, kids, pin, role, content_restrictions }) {
        const store = _load();
        if (store.profiles.length >= FAMILY_PACK_MAX) throw new Error('family_pack_full');
        const p = {
            id: _uuid(),
            name, role: role || 'family',
            initials: initials || (name?.[0] || '?').toUpperCase(),
            gradient: gradient || 'gradient-v',
            kids: !!kids, pin_hash: null,
            autoplay_slides: true, autoplay_previews: true,
            face_group_consent: false,
            default_save: 'this-profile',
            content_restrictions: content_restrictions || (kids ? '13+' : 'none'),
            language: 'en-GB',
            delete_forbidden: false,
            created_at: new Date().toISOString(),
        };
        if (store.profiles.length === 0) { p.role = 'owner'; p.delete_forbidden = true; }
        if (pin) {
            _validatePin(pin);
            const salt = _randomSalt();
            const hash = await _pbkdf2(pin, salt);
            p.pin_hash = `pbkdf2$sha256$${PBKDF2_ITERS}$${salt}$${hash}`;
        }
        store.profiles.push(p);
        if (!store.active_profile_id) store.active_profile_id = p.id;
        _save(store);
        return _view(p);
    },

    async update(profileId, patch) {
        const store = _load();
        const p = store.profiles.find(x => x.id === profileId);
        if (!p) throw new Error('profile_not_found');
        const editable = [
            'name', 'initials', 'gradient', 'kids',
            'autoplay_slides', 'autoplay_previews',
            'face_group_consent', 'default_save', 'content_restrictions', 'language',
        ];
        for (const k of editable) {
            if (patch[k] !== undefined) p[k] = patch[k];
        }
        _save(store);
        return _view(p);
    },

    async remove(profileId) {
        const store = _load();
        const i = store.profiles.findIndex(x => x.id === profileId);
        if (i < 0) throw new Error('profile_not_found');
        if (store.profiles[i].delete_forbidden) throw new Error('owner_locked');
        store.profiles.splice(i, 1);
        if (store.active_profile_id === profileId) {
            store.active_profile_id = store.profiles[0]?.id ?? null;
        }
        _save(store);
    },

    async setPin(profileId, newPin /* null clears */) {
        const store = _load();
        const p = store.profiles.find(x => x.id === profileId);
        if (!p) throw new Error('profile_not_found');
        if (newPin === null || newPin === '') {
            p.pin_hash = null;
        } else {
            _validatePin(newPin);
            const salt = _randomSalt();
            const hash = await _pbkdf2(newPin, salt);
            p.pin_hash = `pbkdf2$sha256$${PBKDF2_ITERS}$${salt}$${hash}`;
        }
        _save(store);
        return _view(p);
    },

    async verifyPin(profileId, pin) {
        const store = _load();
        const p = store.profiles.find(x => x.id === profileId);
        if (!p) throw new Error('profile_not_found');
        if (!p.pin_hash) return true;
        const [, alg, iters, salt, expected] = p.pin_hash.split('$');
        if (alg !== 'sha256') return false;
        // Currently only one iters value; still parse in case we bump later.
        const actual = await _pbkdf2(pin, salt);
        // Timing-safe compare
        if (actual.length !== expected.length) return false;
        let diff = 0;
        for (let i = 0; i < actual.length; i++) {
            diff |= actual.charCodeAt(i) ^ expected.charCodeAt(i);
        }
        return diff === 0 && iters === String(PBKDF2_ITERS);
    },

    /** Wipe everything — dev button only. */
    async _wipe() { localStorage.removeItem(STORE_KEY); },
};

// Expose on window for the mock's inline handlers to reach.
window.ProfileStore = ProfileStore;
