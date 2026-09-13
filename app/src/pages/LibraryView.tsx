import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { convertFileSrc } from "@tauri-apps/api/core";
import { LicenceBadge } from "./LicenceBadge";

type CatalogState = {
  ready: boolean;
  library_path: string | null;
  photo_count: number;
  last_scan_at: string | null;
};

type Thumb = {
  id: string;
  path: string;
  taken_at: string | null;
  width: number;
  height: number;
};

export function LibraryView({
  state,
  onRefresh,
  onRescan,
}: {
  state: CatalogState;
  onRefresh: () => void;
  onRescan: () => void;
}) {
  const [recent, setRecent] = useState<Thumb[]>([]);
  const [hits, setHits] = useState<Thumb[] | null>(null);
  const [busy, setBusy] = useState(false);
  const [q, setQ] = useState("");
  const [selected, setSelected] = useState<Thumb | null>(null);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      const rows = await invoke<Thumb[]>("catalog_recent", { limit: 400 });
      if (!cancelled) setRecent(rows);
    })();
    return () => {
      cancelled = true;
    };
  }, [state.photo_count]);

  // Debounced search — 300 ms after user stops typing
  useEffect(() => {
    if (!q.trim()) {
      setHits(null);
      return;
    }
    const t = setTimeout(async () => {
      try {
        const rows = await invoke<Thumb[]>("search_text", { q, limit: 400 });
        setHits(rows);
      } catch (_) {}
    }, 280);
    return () => clearTimeout(t);
  }, [q]);

  const shown = useMemo(() => hits ?? recent, [hits, recent]);

  const rescan = async () => {
    if (!state.library_path) return;
    setBusy(true);
    try {
      await invoke("library_scan", { path: state.library_path });
      onRefresh();
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="library">
      <header className="top-bar">
        <div className="brand">SnapIT</div>
        <LicenceBadge />
        <input
          className="search"
          value={q}
          onChange={(e) => setQ(e.target.value)}
          placeholder="Search filename, camera, date… (semantic search lands in M3-late)"
        />
        <div className="stats">
          {hits ? (
            <>
              {hits.length.toLocaleString()} hit{hits.length === 1 ? "" : "s"}
              {" of "}
              {state.photo_count.toLocaleString()}
            </>
          ) : (
            <>{state.photo_count.toLocaleString()} photos</>
          )}
        </div>
        <div className="actions">
          <button onClick={rescan} disabled={busy}>
            {busy ? "Scanning…" : "Rescan"}
          </button>
          <button onClick={onRescan}>Change library</button>
        </div>
      </header>
      <div className="lib-path-strip" title={state.library_path ?? ""}>
        {state.library_path}
      </div>
      <main className="grid">
        {shown.length === 0 ? (
          <div className="empty-grid muted">
            {hits !== null
              ? "No hits."
              : "No photos indexed yet. If you just picked the folder, hit Rescan."}
          </div>
        ) : (
          shown.map((t) => (
            <PhotoCell key={t.id} t={t} onOpen={() => setSelected(t)} />
          ))
        )}
      </main>
      {selected && (
        <PhotoDetail
          thumb={selected}
          onClose={() => setSelected(null)}
          onNext={() => {
            const list = shown;
            const i = list.findIndex((x) => x.id === selected.id);
            if (i >= 0 && i + 1 < list.length) setSelected(list[i + 1]);
          }}
          onPrev={() => {
            const list = shown;
            const i = list.findIndex((x) => x.id === selected.id);
            if (i > 0) setSelected(list[i - 1]);
          }}
        />
      )}
    </div>
  );
}

function PhotoCell({ t, onOpen }: { t: Thumb; onOpen: () => void }) {
  const [src, setSrc] = useState<string | null>(null);
  const [err, setErr] = useState(false);
  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const abs = await invoke<string>("thumb_ensure", { photoId: t.id });
        if (!cancelled) setSrc(convertFileSrc(abs));
      } catch (_e) {
        if (!cancelled) setErr(true);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [t.id]);
  const name = t.path.split(/[\\/]/).pop() || t.path;
  return (
    <div className="cell" title={t.path} onDoubleClick={onOpen} onClick={onOpen}>
      <div className="cell-img">
        {src ? (
          <img src={src} alt={name} loading="lazy" />
        ) : err ? (
          <div className="cell-err">×</div>
        ) : (
          <div className="cell-skel" />
        )}
      </div>
      <div className="cell-caption">
        <div className="cell-name">{name}</div>
        <div className="cell-meta">
          {t.width && t.height ? (
            <span className="cell-dim">
              {t.width}×{t.height}
            </span>
          ) : null}
          {t.taken_at && (
            <span className="cell-date">
              {new Date(t.taken_at).toLocaleDateString()}
            </span>
          )}
        </div>
      </div>
    </div>
  );
}

function PhotoDetail({
  thumb,
  onClose,
  onNext,
  onPrev,
}: {
  thumb: Thumb;
  onClose: () => void;
  onNext: () => void;
  onPrev: () => void;
}) {
  const [big, setBig] = useState<string | null>(null);
  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const abs = await invoke<string>("thumb_ensure", { photoId: thumb.id });
        if (!cancelled) setBig(convertFileSrc(abs));
      } catch (_) {}
    })();
    return () => {
      cancelled = true;
    };
  }, [thumb.id]);

  useEffect(() => {
    const h = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
      if (e.key === "ArrowRight") onNext();
      if (e.key === "ArrowLeft") onPrev();
    };
    window.addEventListener("keydown", h);
    return () => window.removeEventListener("keydown", h);
  }, [onClose, onNext, onPrev]);

  const name = thumb.path.split(/[\\/]/).pop() || thumb.path;

  return (
    <div className="detail-overlay" onClick={onClose}>
      <div className="detail-inner" onClick={(e) => e.stopPropagation()}>
        <div className="detail-img">
          {big && <img src={big} alt={name} />}
          <button className="detail-nav prev" onClick={onPrev} aria-label="Previous">
            ‹
          </button>
          <button className="detail-nav next" onClick={onNext} aria-label="Next">
            ›
          </button>
          <button className="detail-close" onClick={onClose} aria-label="Close">
            ×
          </button>
        </div>
        <aside className="detail-meta">
          <h3>{name}</h3>
          <div className="detail-path">{thumb.path}</div>
          <dl>
            <dt>Dimensions</dt>
            <dd>
              {thumb.width}×{thumb.height}
            </dd>
            {thumb.taken_at && (
              <>
                <dt>Taken</dt>
                <dd>{new Date(thumb.taken_at).toLocaleString()}</dd>
              </>
            )}
          </dl>
          <p className="muted small">
            Edit stack + face clusters + upscale land in later R·01 milestones.
            Everything on this panel lives inside your library folder — nothing
            has been sent anywhere.
          </p>
        </aside>
      </div>
    </div>
  );
}
