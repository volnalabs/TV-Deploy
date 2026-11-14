import { CircleHelp, Plus, Search, Tv2 } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useApp } from "../app/context";
import { DiagnosisPanel } from "../components/DiagnosisPanel";
import { Button, Modal, Progress } from "../components/ui";
import { useI18n } from "../hooks/i18n";
import { looksLikeIpv4, opId } from "../lib/format";
import { api, on, toApiError } from "../services/api";
import type { Diagnosis, TvInfo } from "../types";
import { DevModeGuide } from "./DevModeGuide";

interface Entry { ip: string; port: number; name: string | null; live: boolean }

/** Search the network, add by address, pick a saved TV — and connect to it. */
export function TvFinder({ onConnected, showSaved = true }: { onConnected: (tv: TvInfo) => void; showSaved?: boolean }) {
  const { t } = useI18n();
  const { settings, refresh } = useApp();
  const [found, setFound] = useState<{ ip: string; port: number; name: string | null }[] | null>(null);
  const [scanning, setScanning] = useState(false);
  const [scanFraction, setScanFraction] = useState<number | null>(null);
  const [connectingKey, setConnectingKey] = useState<string | null>(null);
  const [manual, setManual] = useState("");
  const [manualError, setManualError] = useState(false);
  const [problem, setProblem] = useState<{ d: Diagnosis; technical: string | null; hostIp: string | null } | null>(null);
  const [guide, setGuide] = useState(false);
  const op = useRef<string | null>(null);

  useEffect(() => {
    const off = on<{ op_id: string | null; fraction: number }>("scan-progress", (p) => { if (p.op_id === op.current) setScanFraction(p.fraction); });
    return () => { void off.then((f) => f()); };
  }, []);

  const entries = useMemo<Entry[]>(() => {
    const map = new Map<string, Entry>();
    if (showSaved) for (const s of settings.devices) map.set(`${s.ip}:${s.port}`, { ip: s.ip, port: s.port, name: s.name, live: false });
    for (const f of found ?? []) {
      const k = `${f.ip}:${f.port}`;
      map.set(k, { ip: f.ip, port: f.port, name: settings.devices.find((d) => d.ip === f.ip && d.port === f.port)?.name ?? f.name, live: true });
    }
    return [...map.values()];
  }, [settings.devices, found, showSaved]);

  const scan = useCallback(async () => {
    const id = opId();
    op.current = id;
    setScanning(true); setProblem(null); setScanFraction(null);
    try {
      const r = await api.scanTvs(id);
      if (r.ok && r.data) setFound(r.data.devices);
      else if (r.diagnosis && r.diagnosis !== "cancelled") setProblem({ d: r.diagnosis, technical: r.technical, hostIp: null });
    } catch (e) {
      const code = toApiError(e).code;
      if (code !== "cancelled") setProblem({ d: "unknown", technical: code, hostIp: null });
    } finally {
      setScanning(false); op.current = null;
    }
  }, []);

  const connect = useCallback(async (ip: string, port: number | null) => {
    const id = opId();
    op.current = id;
    setConnectingKey(`${ip}:${port ?? ""}`); setProblem(null);
    try {
      const r = await api.connectTv(ip, port, id);
      if (r.ok && r.data) {
        await refresh();
        onConnected(r.data);
      } else if (r.diagnosis && r.diagnosis !== "cancelled") {
        setProblem({ d: r.diagnosis, technical: r.technical, hostIp: r.host_ip });
      }
    } catch (e) {
      const code = toApiError(e).code;
      if (code === "invalid_ip" || code === "forbidden_ip") setManualError(true);
      else if (code !== "cancelled") setProblem({ d: "unknown", technical: code, hostIp: null });
    } finally {
      setConnectingKey(null); op.current = null;
    }
  }, [onConnected, refresh]);

  const submitManual = () => {
    if (!looksLikeIpv4(manual)) { setManualError(true); return; }
    setManualError(false);
    void connect(manual.trim(), null);
  };
  const busy = scanning || !!connectingKey;

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-end gap-x-3 gap-y-3">
        <Button variant="primary" busy={scanning} disabled={!!connectingKey} onClick={scan}><Search size={16} aria-hidden />{scanning ? t("tvFinding") : t("tvFind")}</Button>
        {busy && <Button variant="ghost" onClick={() => op.current && void api.cancel(op.current)}>{t("cancel")}</Button>}
        <form className="ml-auto flex min-w-0 flex-1 basis-64 items-center gap-2 sm:flex-none" onSubmit={(e) => { e.preventDefault(); submitManual(); }}>
          <input value={manual} onChange={(e) => { setManual(e.target.value); setManualError(false); }} placeholder={t("tvIpPlaceholder")} aria-label={t("tvIpLabel")}
            aria-invalid={manualError} inputMode="decimal" autoComplete="off" className="input min-w-0 flex-1 sm:w-44" />
          <Button type="submit" busy={connectingKey === `${manual.trim()}:`} disabled={busy}><Plus size={16} aria-hidden />{t("tvAddByIp")}</Button>
        </form>
      </div>
      {scanning && <Progress value={scanFraction} label={t("tvFinding")} />}
      {manualError && <p role="alert" className="text-[var(--err)]">{t("tvInvalidIp")}</p>}

      {entries.length > 0 && (
        <ul className="m-0 list-none divide-y divide-[var(--border)] overflow-hidden rounded-xl border border-[var(--border)] p-0">
          {entries.map((e) => {
            const key = `${e.ip}:${e.port}`;
            return (
              <li key={key} className="fade-up flex items-center gap-3 px-3.5 py-3">
                <span className="relative grid h-9 w-9 shrink-0 place-items-center rounded-lg bg-[var(--surface-2)] text-[var(--muted)]">
                  <Tv2 size={18} aria-hidden />
                  {e.live && <span className="absolute -top-0.5 -right-0.5 h-2.5 w-2.5 rounded-full border-2 border-[var(--surface)] bg-[var(--ok)]" />}
                </span>
                <div className="min-w-0 flex-1">
                  <p className="truncate font-medium">{e.name ?? "Samsung TV"}</p>
                  <p className="selectable truncate text-xs text-[var(--muted)]">{e.ip} · {e.live ? t("tvFound") : t("tvSaved")}</p>
                </div>
                <Button size="sm" busy={connectingKey === key} disabled={busy} onClick={() => connect(e.ip, e.port)}>{connectingKey === key ? t("tvConnecting") : t("tvConnect")}</Button>
              </li>
            );
          })}
        </ul>
      )}
      {found !== null && found.length === 0 && !scanning && !problem && <p className="text-[var(--muted)]">{t("tvNoneFound")}</p>}
      {problem && <DiagnosisPanel diagnosis={problem.d} technical={problem.technical} hostIp={problem.hostIp} />}
      <button type="button" onClick={() => setGuide(true)} className="inline-flex items-center gap-1.5 text-[13px] font-medium text-[var(--accent)] hover:underline">
        <CircleHelp size={15} aria-hidden />{t("tvHelp")}
      </button>
      {guide && <Modal title={t("devTitle")} onClose={() => setGuide(false)} wide><DevModeGuide /></Modal>}
    </div>
  );
}
