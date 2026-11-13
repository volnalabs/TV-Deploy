import { open } from "@tauri-apps/plugin-dialog";
import { FolderOpen, Wrench } from "lucide-react";
import { useEffect, useState } from "react";
import { Button } from "../components/ui";
import { useI18n } from "../hooks/i18n";
import type { Key } from "../i18n";
import { api, toApiError } from "../services/api";
import type { ToolchainStatus } from "../types";

/** Shown while the Samsung tools can't be used yet. */
export function SetupPanel({ status, onReady }: { status: ToolchainStatus; onReady: () => void }) {
  const { t } = useI18n();
  const [found, setFound] = useState<string[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<Key | null>(null);

  useEffect(() => {
    api.findExistingSdk().then(setFound).catch(() => setFound([]));
  }, []);

  const apply = async (path: string) => {
    setBusy(true);
    setError(null);
    try {
      await api.setSdkPath(path);
      onReady();
    } catch (e) {
      const code = toApiError(e).code;
      setError(code === "sdk_invalid" ? "setupInvalid" : code === "sdk_init_failed" ? "err_sdk_init_failed" : "err_internal");
    } finally {
      setBusy(false);
    }
  };

  const choose = async () => {
    const p = await open({ directory: true, multiple: false });
    if (typeof p === "string") await apply(p);
  };

  const brokenBuild = status.node === "missing" ? "setupNodeMissing" : !status.cli_ready ? "setupCliMissing" : null;

  return (
    <div className="fade-up rounded-xl border border-[var(--border)] bg-[var(--surface)] p-5">
      <div className="flex items-start gap-3">
        <Wrench size={18} className="mt-0.5 text-[var(--accent)]" aria-hidden />
        <div className="min-w-0 flex-1">
          <p className="m-0 font-semibold">{t("setupTitle")}</p>
          <p className="m-0 mt-1 text-[var(--muted)]">{t("setupBody")}</p>
          {brokenBuild && <p className="m-0 mt-3 text-[var(--err)]">{t(brokenBuild)}</p>}
          {found.length > 0 && (
            <div className="mt-4">
              <p className="m-0 mb-2 text-xs font-medium uppercase tracking-wide text-[var(--muted)]">{t("setupFoundSdk")}</p>
              {found.map((p) => (
                <div key={p} className="flex items-center gap-3 border-t border-[var(--border)] py-2">
                  <span className="selectable min-w-0 flex-1 truncate">{p}</span>
                  <Button variant="primary" busy={busy} disabled={!!brokenBuild} onClick={() => apply(p)}>{t("setupUse")}</Button>
                </div>
              ))}
            </div>
          )}
          <div className="mt-4 flex items-center gap-3">
            <Button busy={busy} disabled={!!brokenBuild} onClick={choose}><FolderOpen size={15} aria-hidden />{t("setupChoose")}</Button>
          </div>
          {error && <p role="alert" className="m-0 mt-3 text-[var(--err)]">{t(error)}</p>}
          <p className="m-0 mt-4 text-xs text-[var(--muted)]">{t("setupDiskNote", { gb: status.sdk_required_gb })}</p>
        </div>
      </div>
    </div>
  );
}
