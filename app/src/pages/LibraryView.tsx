import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { convertFileSrc } from "@tauri-apps/api/core";
import { save as saveDialog } from "@tauri-apps/plugin-dialog";
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
  xmp_rating: number | null;
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
  const [showDupes, setShowDupes] = useState(false);

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
          <button onClick={() => setShowDupes(true)}>Duplicates</button>
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
      {showDupes && <DuplicatesPanel onClose={() => setShowDupes(false)} />}
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
        {t.xmp_rating != null && t.xmp_rating > 0 && (
          <div className="cell-rating" title={`${t.xmp_rating}★`}>
            {"★".repeat(t.xmp_rating)}
          </div>
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

type DupeGroup = {
  content_hash: string;
  bytes_each: number;
  copies: Thumb[];
};

function DuplicatesPanel({ onClose }: { onClose: () => void }) {
  const [groups, setGroups] = useState<DupeGroup[] | null>(null);
  const [err, setErr] = useState<string | null>(null);

  useEffect(() => {
    (async () => {
      try {
        const g = await invoke<DupeGroup[]>("catalog_duplicates", { limit: 500 });
        setGroups(g);
      } catch (e: any) {
        setErr(String(e?.message ?? e));
      }
    })();
  }, []);

  useEffect(() => {
    const h = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", h);
    return () => window.removeEventListener("keydown", h);
  }, [onClose]);

  const wasted = useMemo(() => {
    if (!groups) return 0;
    return groups.reduce((n, g) => n + g.bytes_each * (g.copies.length - 1), 0);
  }, [groups]);

  return (
    <div className="detail-overlay" onClick={onClose}>
      <div
        className="dupe-panel"
        onClick={(e) => e.stopPropagation()}
      >
        <header className="dupe-head">
          <div>
            <h2>Duplicates</h2>
            <div className="muted small">
              Byte-identical copies across your library.
              {groups && groups.length > 0 && (
                <> Removing extras would free about {fmtBytes(wasted)}.</>
              )}
            </div>
          </div>
          <button className="detail-close" onClick={onClose} aria-label="Close">
            ×
          </button>
        </header>
        <div className="dupe-body">
          {err && <div className="dupe-err">{err}</div>}
          {!groups && !err && <div className="muted">Scanning…</div>}
          {groups && groups.length === 0 && (
            <div className="muted">
              None found. Every photo in your library is unique.
            </div>
          )}
          {groups?.map((g) => (
            <div key={g.content_hash} className="dupe-group">
              <div className="dupe-group-head">
                <span className="mono">{g.content_hash.slice(0, 12)}…</span>
                <span className="muted">
                  {g.copies.length} copies • {fmtBytes(g.bytes_each)} each
                </span>
              </div>
              <div className="dupe-copies">
                {g.copies.map((c) => (
                  <DupeCell key={c.id} t={c} />
                ))}
              </div>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}

function DupeCell({ t }: { t: Thumb }) {
  const [src, setSrc] = useState<string | null>(null);
  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const abs = await invoke<string>("thumb_ensure", { photoId: t.id });
        if (!cancelled) setSrc(convertFileSrc(abs));
      } catch {}
    })();
    return () => {
      cancelled = true;
    };
  }, [t.id]);
  return (
    <div className="dupe-cell" title={t.path}>
      <div className="dupe-thumb">{src && <img src={src} alt="" />}</div>
      <div className="dupe-path">{t.path}</div>
    </div>
  );
}

function fmtBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  if (n < 1024 * 1024 * 1024) return `${(n / 1024 / 1024).toFixed(1)} MB`;
  return `${(n / 1024 / 1024 / 1024).toFixed(2)} GB`;
}

type ExportReport = {
  written_path: string;
  bytes: number;
  width: number;
  height: number;
  applied_ops: number;
};

function ExportPanel({ photoId, filename }: { photoId: string; filename: string }) {
  const [quality, setQuality] = useState(92);
  const [maxEdge, setMaxEdge] = useState<number | "">("");
  const [busy, setBusy] = useState(false);
  const [report, setReport] = useState<ExportReport | null>(null);
  const [err, setErr] = useState<string | null>(null);

  const suggestedName = useMemo(() => {
    const base = filename.replace(/\.[^.]+$/, "");
    return `${base}.snapit.jpg`;
  }, [filename]);

  const doExport = async () => {
    setErr(null);
    setReport(null);
    setBusy(true);
    try {
      const dest = await saveDialog({
        defaultPath: suggestedName,
        filters: [{ name: "JPEG", extensions: ["jpg", "jpeg"] }],
      });
      if (!dest) {
        setBusy(false);
        return;
      }
      const req = {
        photo_id: photoId,
        destination: dest,
        quality,
        ...(maxEdge && Number(maxEdge) > 0 ? { max_edge: Number(maxEdge) } : {}),
      };
      const r = await invoke<ExportReport>("edit_export", { request: req });
      setReport(r);
    } catch (e: any) {
      setErr(String(e?.message ?? e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="export-panel">
      <div className="export-title">Export</div>
      <label className="export-row">
        <span>JPEG quality</span>
        <input
          type="range"
          min={40}
          max={100}
          step={1}
          value={quality}
          onChange={(e) => setQuality(Number(e.target.value))}
        />
        <span className="export-val">{quality}</span>
      </label>
      <label className="export-row">
        <span>Max edge</span>
        <input
          type="number"
          className="export-num"
          placeholder="Full size"
          min={256}
          max={12000}
          value={maxEdge}
          onChange={(e) =>
            setMaxEdge(e.target.value === "" ? "" : Number(e.target.value))
          }
        />
        <span className="export-val muted">px</span>
      </label>
      <button className="pri export-btn" onClick={doExport} disabled={busy}>
        {busy ? "Exporting…" : "Export JPEG"}
      </button>
      {report && (
        <div className="export-ok">
          Wrote {report.width}×{report.height} • {(report.bytes / 1024).toFixed(0)} KB
          {report.applied_ops > 0 && ` • ${report.applied_ops} edit${report.applied_ops === 1 ? "" : "s"} applied`}
        </div>
      )}
      {err && <div className="export-err">{err}</div>}
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
          <ExportPanel photoId={thumb.id} filename={name} />
          <p className="muted small">
            Face clusters + semantic search + upscale land in later R·01
            milestones. Everything on this panel lives inside your library
            folder — nothing has been sent anywhere.
          </p>
        </aside>
      </div>
    </div>
  );
}
