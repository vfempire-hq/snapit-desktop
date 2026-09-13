import { useEffect, useState } from "react";
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
  const [thumbs, setThumbs] = useState<Thumb[]>([]);
  const [busy, setBusy] = useState(false);
  const [q, setQ] = useState("");

  useEffect(() => {
    let cancelled = false;
    (async () => {
      const rows = await invoke<Thumb[]>("catalog_recent", { limit: 400 });
      if (!cancelled) setThumbs(rows);
    })();
    return () => {
      cancelled = true;
    };
  }, [state.photo_count]);

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

  const filtered = q.trim()
    ? thumbs.filter((t) => t.path.toLowerCase().includes(q.trim().toLowerCase()))
    : thumbs;

  return (
    <div className="library">
      <header className="top-bar">
        <div className="brand">SnapIT</div>
        <LicenceBadge />
        <input
          className="search"
          value={q}
          onChange={(e) => setQ(e.target.value)}
          placeholder="Search filename (semantic search lands in R·01 M3)"
        />
        <div className="stats">
          {state.photo_count.toLocaleString()} photos
          {state.last_scan_at && (
            <span className="muted" style={{ marginLeft: 12 }}>
              last scan {new Date(state.last_scan_at).toLocaleString()}
            </span>
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
        {filtered.length === 0 ? (
          <div className="empty-grid muted">
            {thumbs.length === 0
              ? "No photos indexed yet. If you just picked the folder, hit Rescan."
              : "No matches for that filter."}
          </div>
        ) : (
          filtered.map((t) => <PhotoCell key={t.id} t={t} />)
        )}
      </main>
    </div>
  );
}

function PhotoCell({ t }: { t: Thumb }) {
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
    <div className="cell" title={t.path}>
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
