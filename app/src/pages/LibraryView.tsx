import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

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

  useEffect(() => {
    let cancelled = false;
    (async () => {
      const rows = await invoke<Thumb[]>("catalog_recent", { limit: 200 });
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

  return (
    <div className="library">
      <header className="top-bar">
        <div className="brand">SnapIT</div>
        <div className="lib-path" title={state.library_path ?? ""}>
          {state.library_path}
        </div>
        <div className="stats">
          {state.photo_count.toLocaleString()} photos
          {state.last_scan_at && (
            <span className="muted" style={{ marginLeft: 12 }}>
              last scan {new Date(state.last_scan_at).toLocaleString()}
            </span>
          )}
        </div>
        <div className="actions">
          <button onClick={rescan} disabled={busy}>{busy ? "Scanning…" : "Rescan"}</button>
          <button onClick={onRescan}>Change library</button>
        </div>
      </header>
      <main className="grid">
        {thumbs.length === 0 ? (
          <div className="empty-grid muted">
            No photos indexed yet. If you just picked the folder, hit <b>Rescan</b>.
          </div>
        ) : (
          thumbs.map((t) => (
            <div key={t.id} className="cell" title={t.path}>
              <div className="cell-inner">
                <div className="cell-meta">
                  <div className="cell-dim">
                    {t.width}×{t.height}
                  </div>
                  <div className="cell-date">
                    {t.taken_at ? new Date(t.taken_at).toLocaleDateString() : ""}
                  </div>
                </div>
              </div>
            </div>
          ))
        )}
      </main>
    </div>
  );
}
