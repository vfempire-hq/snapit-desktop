import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { readTextFile } from "@tauri-apps/plugin-fs";
import { open as openShell } from "@tauri-apps/plugin-shell";

type LicenceStatus = {
  activated: boolean;
  email: string | null;
  tier: string | null;
  issued_at: number | null;
  major_version: number | null;
  trial_days_remaining: number | null;
};

export function LicenceBadge() {
  const [st, setSt] = useState<LicenceStatus | null>(null);
  const [importing, setImporting] = useState(false);
  const [err, setErr] = useState<string | null>(null);

  const refresh = async () => setSt(await invoke<LicenceStatus>("licence_status"));

  useEffect(() => {
    refresh();
  }, []);

  const importLicence = async () => {
    setErr(null);
    setImporting(true);
    try {
      const licJson = await openDialog({
        multiple: false,
        directory: false,
        title: "Pick snapit.licence.json",
        filters: [{ name: "SnapIT licence", extensions: ["json"] }],
      });
      if (!licJson || typeof licJson !== "string") return;
      const sig = await openDialog({
        multiple: false,
        directory: false,
        title: "Pick snapit.licence.sig",
        filters: [{ name: "SnapIT signature", extensions: ["sig", "txt"] }],
      });
      if (!sig || typeof sig !== "string") return;

      const [licenceText, signatureText] = await Promise.all([
        readTextFile(licJson),
        readTextFile(sig),
      ]);
      await invoke("licence_import", {
        licenceJson: licenceText.trim(),
        signatureB64: signatureText.trim(),
      });
      await refresh();
    } catch (e: any) {
      setErr(e?.toString() ?? "import failed");
    } finally {
      setImporting(false);
    }
  };

  const forget = async () => {
    if (!confirm("Forget the licence on this device? You can re-import at any time.")) return;
    await invoke("licence_forget");
    await refresh();
  };

  const buy = () => openShell("https://snapit.vfempire.com/#buy");

  if (!st) return null;

  if (st.activated) {
    return (
      <div className="lic activated" title={`Issued ${st.issued_at ? new Date(st.issued_at * 1000).toLocaleDateString() : ""}`}>
        <span className="lic-dot" />
        <span className="lic-tier">{st.tier?.toUpperCase()}</span>
        <span className="lic-email">{st.email}</span>
        <button className="lic-forget" onClick={forget} title="Forget this licence on this device">
          ×
        </button>
      </div>
    );
  }

  const trial = st.trial_days_remaining ?? 14;
  return (
    <div className={"lic " + (trial > 0 ? "trial" : "expired")}>
      <span className="lic-dot" />
      <span className="lic-tier">
        {trial > 0 ? `TRIAL · ${trial} day${trial === 1 ? "" : "s"} left` : "TRIAL ENDED"}
      </span>
      <button className="lic-import" onClick={importLicence} disabled={importing}>
        {importing ? "…" : "Import licence"}
      </button>
      <button className="lic-buy" onClick={buy}>
        Buy €69
      </button>
      {err && <span className="lic-err">{err}</span>}
    </div>
  );
}
