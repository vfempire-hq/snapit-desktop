import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { LibraryView } from "./pages/LibraryView";

type CatalogState = {
  ready: boolean;
  library_path: string | null;
  photo_count: number;
  last_scan_at: string | null;
};

type ScanProgress = { kind: string; total: number; done: number };

export function App() {
  const [state, setState] = useState<CatalogState | null>(null);
  const [scanning, setScanning] = useState(false);
  const [progress, setProgress] = useState<ScanProgress | null>(null);

  const refresh = async () => {
    try {
      const st = await invoke<CatalogState>("catalog_state");
      setState(st);
    } catch (e) {
      console.error("catalog_state failed", e);
    }
  };

  useEffect(() => {
    refresh();
    const unlisten = listen<ScanProgress>("snapit://scan/progress", (ev) => {
      setProgress(ev.payload);
      if (ev.payload.kind === "finish") {
        // Refresh catalog stats when scan actually settles.
        setTimeout(() => {
          refresh();
          setProgress(null);
        }, 400);
      }
    });
    return () => {
      unlisten.then((u) => u()).catch(() => {});
    };
  }, []);

  const pickLibrary = async () => {
    const picked = await open({
      directory: true,
      multiple: false,
      title: "Pick a folder to make your SnapIT library",
    });
    if (!picked || typeof picked !== "string") return;
    setScanning(true);
    try {
      await invoke("catalog_open", { path: picked });
      await invoke("library_scan", { path: picked });
      await refresh();
    } finally {
      setScanning(false);
    }
  };

  if (!state) return <BootScreen />;

  if (!state.library_path) {
    return <EmptyState onPick={pickLibrary} scanning={scanning} progress={progress} />;
  }

  return (
    <>
      <LibraryView state={state} onRefresh={refresh} onRescan={pickLibrary} />
      {progress && progress.kind !== "finish" && <ProgressBar p={progress} />}
    </>
  );
}

function ProgressBar({ p }: { p: ScanProgress }) {
  const pct = p.total > 0 ? Math.min(100, Math.round((p.done / p.total) * 100)) : 0;
  return (
    <div className="scan-toast">
      <div className="scan-title">Scanning library…</div>
      <div className="scan-track">
        <div className="scan-fill" style={{ width: pct + "%" }} />
      </div>
      <div className="scan-meta">
        {p.done.toLocaleString()} / {p.total.toLocaleString()} · {pct}%
      </div>
    </div>
  );
}

function BootScreen() {
  return (
    <div className="boot">
      <h1>SnapIT</h1>
      <p className="muted">Loading catalog…</p>
    </div>
  );
}

function EmptyState({
  onPick,
  scanning,
  progress,
}: {
  onPick: () => void;
  scanning: boolean;
  progress: ScanProgress | null;
}) {
  return (
    <div className="empty">
      <div className="empty-card">
        <div className="logo">
          <svg viewBox="0 0 64 64" width="72" height="72" fill="none" stroke="currentColor" strokeWidth="1.6">
            <rect x="6" y="14" width="52" height="42" rx="6" />
            <circle cx="32" cy="35" r="12" />
            <circle cx="32" cy="35" r="5" fill="currentColor" />
            <rect x="22" y="9" width="20" height="7" rx="2" />
          </svg>
        </div>
        <h1>SnapIT</h1>
        <p className="sub">Own your photo library. On your storage. One price.</p>
        <button className="pri" onClick={onPick} disabled={scanning}>
          {scanning ? "Scanning…" : "Pick a folder to start your library"}
        </button>
        {progress && progress.kind !== "finish" && (
          <div className="scan-inline">
            <div className="scan-track" style={{ margin: "18px 0 8px" }}>
              <div
                className="scan-fill"
                style={{
                  width:
                    progress.total > 0
                      ? Math.min(100, (progress.done / progress.total) * 100) + "%"
                      : "0%",
                }}
              />
            </div>
            <div className="muted">
              {progress.done.toLocaleString()} / {progress.total.toLocaleString()} photos indexed
            </div>
          </div>
        )}
        <p className="fine">
          Your library never leaves this device unless you export it. On-device AI. No cloud.
        </p>
      </div>
    </div>
  );
}
