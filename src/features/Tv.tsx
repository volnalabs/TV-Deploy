import { CheckCircle2, Plus, RefreshCw, Search, Tv2 } from "lucide-react";
import { useCallback, useMemo, useState } from "react";
import { DiagnosisPanel } from "../components/DiagnosisPanel";
import { Button, Pill, Row } from "../components/ui";
import { useI18n } from "../hooks/i18n";
import { looksLikeIpv4, opId } from "../lib/format";
import { api, toApiError } from "../services/api";
import type { Diagnosis, SavedDevice, TvInfo } from "../types";

interface Entry { ip: string; port: number; name: string | null; live: boolean }

export function TvPanel({ saved, tv, onTv, onSavedChanged }: { saved: SavedDevice[]; tv: TvInfo | null; onTv: (t: TvInfo | null) => void; onSavedChanged: () => void }) {
  const { t } = useI18n();
  const [found, setFound] = useState<{ ip: string; port: number }[] | null>(null);
  const [scanning, setScanning] = useState(false);
  const [connectingKey, setConnectingKey] = useState<string | null>(null);
  const [manual, setManual] = useState("");
  const [manualError, setManualError] = useState(false);
  const [problem, setProblem] = useState<{ d: Diagnosis; technical: string | null; hostIp: string | null } | null>(null);
  const [currentOp, setCurrentOp] = useState<string | null>(null);

  const entries = useMemo<Entry[]>(() => {
    const map = new Map<string, Entry>();
    for (const s of saved) map.set(`${s.ip}:${s.port}`, { ip: s.ip, port: s.port, name: s.name, live: false });
    for (const f of found ?? []) {
      const k = `${f.ip}:${f.port}`;
      map.set(k, { ip: f.ip, port: f.port, name: map.get(k)?.name ?? null, live: true });
    }
    return [...map.values()];
  }, [saved, found]);

  const scan = useCallback(async () => {
    const id = opId();
    setCurrentOp(id);
    setScanning(true);
    setProblem(null);
    try {
      const r = await api.scanTvs(id);
      if (r.ok && r.data) setFound(r.data.devices.map((d) => ({ ip: d.ip, port: d.port })));
      else if (r.diagnosis && r.diagnosis !== "cancelled") setProblem({ d: r.diagnosis, technical: r.technical, hostIp: null });
    } catch (e) {
      setProblem({ d: "unknown", technical: toApiError(e).code, hostIp: null });
    } finally {
      setScanning(false);
      setCurrentOp(null);
    }
  }, []);

  const connect = useCallback(async (ip: string, port: number | null) => {
    const key = `${ip}:${port ?? ""}`;
    const id = opId();
    setCurrentOp(id);
    setConnectingKey(key);
    setProblem(null);
    try {
      const r = await api.connectTv(ip, port, id);
      if (r.ok && r.data) {
        onTv(r.data);
        onSavedChanged();
      } else if (r.diagnosis && r.diagnosis !== "cancelled") {
        setProblem({ d: r.diagnosis, technical: r.technical, hostIp: r.host_ip });
      }
    } catch (e) {
      const code = toApiError(e).code;
      if (code === "invalid_ip" || code === "forbidden_ip") setManualError(true);
      else setProblem({ d: "unknown", technical: code, hostIp: null });
    } finally {
      setConnectingKey(null);
      setCurrentOp(null);
    }
  }, [onTv, onSavedChanged]);

  const submitManual = () => {
    if (!looksLikeIpv4(manual)) {
      setManualError(true);
      return;
    }
    setManualError(false);
    void connect(manual.trim(), null);
  };

  if (tv) {
    return (
      <div className="fade-up rounded-xl border border-[var(--border)] bg-[var(--surface)] p-4">
        <div className="flex items-center gap-3">
          <span className="grid h-10 w-10 place-items-center rounded-lg bg-[var(--accent-soft)] text-[var(--accent)]"><Tv2 size={20} aria-hidden /></span>
          <div className="min-w-0 flex-1">
            <p className="m-0 truncate font-semibold">{tv.name}</p>
            <Pill tone="ok"><CheckCircle2 size={12} aria-hidden />{t("tvConnected")}</Pill>
          </div>
          <Button onClick={() => onTv(null)}>{t("tvChange")}</Button>
        </div>
        <dl className="m-0 mt-3">
          <Row label="IP">{tv.ip}:{tv.port}</Row>
          {tv.duid && <Row label={t("tvDuid")}>{tv.duid}</Row>}
          {tv.host_ip && <Row label={t("tvHostIp")}>{tv.host_ip}</Row>}
        </dl>
      </div>
    );
  }

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-center gap-2">
        <Button variant="primary" busy={scanning} disabled={!!connectingKey} onClick={scan}><Search size={15} aria-hidden />{scanning ? t("tvFinding") : t("tvFind")}</Button>
        {currentOp && <Button variant="ghost" onClick={() => void api.cancel(currentOp)}>{t("cancel")}</Button>}
        <form className="ml-auto flex items-center gap-2" onSubmit={(e) => { e.preventDefault(); submitManual(); }}>
          <input
            value={manual}
            onChange={(e) => { setManual(e.target.value); setManualError(false); }}
            placeholder={t("tvIpPlaceholder")}
            aria-label={t("tvAddByIp")}
            aria-invalid={manualError}
            inputMode="decimal"
            className={`selectable h-9 w-44 rounded-lg border bg-[var(--surface)] px-3 text-[13px] outline-none focus:border-[var(--accent)] ${manualError ? "border-[var(--err)]" : "border-[var(--border)]"}`}
          />
          <Button type="submit" busy={connectingKey === `${manual.trim()}:`}><Plus size={15} aria-hidden />{t("tvAddByIp")}</Button>
        </form>
      </div>
      {manualError && <p role="alert" className="m-0 text-[var(--err)]">{t("tvInvalidIp")}</p>}

      {entries.length > 0 && (
        <ul className="m-0 list-none divide-y divide-[var(--border)] overflow-hidden rounded-xl border border-[var(--border)] bg-[var(--surface)] p-0">
          {entries.map((e) => {
            const key = `${e.ip}:${e.port}`;
            return (
              <li key={key} className="flex items-center gap-3 px-4 py-3">
                <Tv2 size={18} className="text-[var(--muted)]" aria-hidden />
                <div className="min-w-0 flex-1">
                  <p className="m-0 truncate font-medium">{e.name ?? `Samsung TV`}</p>
                  <p className="selectable m-0 text-xs text-[var(--muted)]">{e.ip}:{e.port} · {e.live ? t("tvFound") : t("tvSaved")}</p>
                </div>
                <Button busy={connectingKey === `${e.ip}:${e.port}`} disabled={!!connectingKey} onClick={() => connect(e.ip, e.port)}>
                  {connectingKey === key ? t("tvConnecting") : e.live ? t("tvConnect") : t("tvReconnect")}
                  {connectingKey !== key && !e.live && <RefreshCw size={13} aria-hidden />}
                </Button>
              </li>
            );
          })}
        </ul>
      )}
      {entries.length > 0 && saved.length > 0 && <p className="m-0 text-xs text-[var(--muted)]">{t("tvSavedHint")}</p>}
      {found !== null && found.length === 0 && !scanning && !problem && <p className="m-0 text-[var(--muted)]">{t("tvNoneFound")}</p>}
      {problem && <DiagnosisPanel diagnosis={problem.d} technical={problem.technical} hostIp={problem.hostIp} />}
    </div>
  );
}
