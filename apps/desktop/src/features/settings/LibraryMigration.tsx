import { useEffect, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { ArrowRight, Check, CircleAlert, DatabaseBackup, HardDrive, LoaderCircle, Server, ShieldCheck } from "lucide-react";
import {
  abandonLibraryMigration, inspectLibraryMigration, onBackendEvent, pendingLibraryMigration, resumeLibraryMigration,
  startLibraryMigration,
} from "../../lib/tauri/api";
import { formatBytes } from "../../lib/format";
import { t } from "../../lib/i18n";
import { ExperimentalBadge } from "../../components/ExperimentalBadge";
import type { BackendConnectionStatus, LibraryMigrationPreflight, LibraryMigrationProgress, PendingLibraryMigration } from "../../types";

interface Props {
  onComplete: (status: BackendConnectionStatus) => void;
  attention?: boolean;
}

export function LibraryMigration({ onComplete, attention = false }: Props) {
  const queryClient = useQueryClient();
  const [open, setOpen] = useState(false);
  const [preflight, setPreflight] = useState<LibraryMigrationPreflight>();
  const [pending, setPending] = useState<PendingLibraryMigration>();
  const [payload, setPayload] = useState("");
  const [allowMissing, setAllowMissing] = useState(false);
  const [loading, setLoading] = useState(false);
  const [running, setRunning] = useState(false);
  const [error, setError] = useState("");
  const [progress, setProgress] = useState<LibraryMigrationProgress>();

  useEffect(() => {
    let active = true;
    let unlisten: (() => void) | undefined;
    void onBackendEvent<LibraryMigrationProgress>("library-migration-progress", (next) => {
      if (active) setProgress(next);
    }).then((stop) => {
      if (!active) stop();
      else unlisten = stop;
    });
    return () => { active = false; unlisten?.(); };
  }, []);

  const inspect = async () => {
    setOpen(true); setLoading(true); setError("");
    try {
      const [next, resumable] = await Promise.all([inspectLibraryMigration(), pendingLibraryMigration()]);
      setPreflight(next); setPending(resumable);
      queryClient.setQueryData(["pending-library-migration"], resumable);
      setAllowMissing(next.stats.missingFiles === 0);
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    } finally { setLoading(false); }
  };

  const run = async (resume: boolean) => {
    setRunning(true); setError("");
    try {
      const status = resume
        ? await resumeLibraryMigration()
        : await startLibraryMigration(payload, allowMissing);
      queryClient.setQueryData(["pending-library-migration"], undefined);
      onComplete(status);
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
      const resumable = await pendingLibraryMigration().catch(() => undefined);
      setPending(resumable);
      queryClient.setQueryData(["pending-library-migration"], resumable);
    } finally { setRunning(false); }
  };
  const abandon = async () => {
    if (!window.confirm(t("Cancel the NAS transfer and discard its resumable snapshot? The local Desktop library will become writable again."))) return;
    setRunning(true); setError("");
    try {
      const status = await abandonLibraryMigration();
      setPending(undefined); setProgress(undefined);
      queryClient.setQueryData(["pending-library-migration"], undefined);
      onComplete(status);
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    } finally { setRunning(false); }
  };

  const percent = progress?.totalBytes
    ? Math.min(100, Math.round(progress.completedBytes / progress.totalBytes * 100))
    : 0;
  if (!open) {
    return <div className={`library-migration-prompt ${attention ? "is-attention" : ""}`}><span><Server size={19} /></span><div><strong>{t(attention ? "Resume interrupted NAS migration" : "Move this library to a NAS")} <ExperimentalBadge /></strong><small>{t(attention ? "The verified transfer can continue without starting over." : "A one-time transfer that makes volumd the main library. This is not a backup.")}</small></div><button className="button button--quiet" onClick={() => void inspect()}><ArrowRight size={15} />{t(attention ? "Review" : "Start")}</button></div>;
  }
  return <section className="library-migration">
    <header><span><DatabaseBackup size={21} /></span><div><strong>{t("Move this library to a NAS")} <ExperimentalBadge /></strong><small>{t("The Desktop library stays intact as a rollback copy. It is never merged with the NAS database.")}</small></div></header>
    {loading ? <div className="migration-loading"><LoaderCircle className="spin" size={18} />{t("Inspecting the local library…")}</div> : preflight && <>
      <div className="migration-stats">
        <span><strong>{preflight.stats.projects}</strong><small>{t("Projects")}</small></span>
        <span><strong>{preflight.stats.files}</strong><small>{t("Original files")}</small></span>
        <span><strong>{preflight.stats.photos}</strong><small>{t("Photos")}</small></span>
        <span><strong>{formatBytes(preflight.stats.totalBytes)}</strong><small>{t("To transfer")}</small></span>
      </div>
      <div className="migration-safety"><ShieldCheck size={18} /><span><strong>{t("The destination NAS must be empty")}</strong><small>{t("If volumd already contains a project, migration is refused. Tags, collections, notes, documentation, photos, and originals move together.")}</small></span></div>
      {preflight.stats.linkedProjects > 0 && <div className="migration-note"><HardDrive size={17} /><span>{t("{count} linked projects will be copied into managed NAS storage. Their current folders are not modified.", { count: preflight.stats.linkedProjects })}</span></div>}
      {preflight.unreadablePhotos.length > 0 && <div className="migration-warning"><CircleAlert size={17} /><span><strong>{t("Project photos need attention")}</strong><small>{t("Restore every missing photo before migrating. Volum will not silently discard documentation.")}</small></span></div>}
      {preflight.missing.length > 0 && <div className="migration-missing"><div><strong>{t("{count} originals are missing", { count: preflight.missing.length })}</strong><small>{t("Reconnect their disks or explicitly continue with metadata-only entries.")}</small></div><ul>{preflight.missing.slice(0, 5).map((file) => <li key={`${file.projectName}-${file.source}`}><span>{file.projectName}</span><code>{file.filename}</code></li>)}</ul>{preflight.missing.length > 5 && <small>{t("And {count} more…", { count: preflight.missing.length - 5 })}</small>}<label className="migration-confirm"><input type="checkbox" checked={allowMissing} onChange={(event) => setAllowMissing(event.target.checked)} /><span>{t("Keep their metadata and mark those originals as missing on the NAS")}</span></label></div>}
      {pending ? <div className="migration-resume"><div><strong>{t("An interrupted migration can be resumed")}</strong><small>{t("The NAS keeps verified chunks, so completed files are not uploaded again.")}</small></div><div className="settings-actions"><button className="button button--primary" disabled={running} onClick={() => void run(true)}>{running ? <LoaderCircle className="spin" size={15} /> : <ArrowRight size={15} />}{t("Resume migration")}</button><button className="button button--quiet" disabled={running} onClick={() => void abandon()}>{t("Abandon migration")}</button></div></div> : <div className="migration-pair"><label className="field"><span>{t("NAS transfer data")}</span><textarea rows={4} value={payload} onChange={(event) => setPayload(event.target.value)} placeholder={t("In the NAS web administrator, choose Move a Desktop library, create a transfer code, and copy it here.")} /></label><button className="button button--primary" disabled={running || !payload.trim() || !preflight.canMigrate || (preflight.stats.missingFiles > 0 && !allowMissing)} onClick={() => void run(false)}>{running ? <LoaderCircle className="spin" size={15} /> : <Server size={15} />}{t("Copy and make NAS primary")}</button></div>}
    </>}
    {error && <div className="form-error" role="alert">{error}</div>}
    {!running && <button className="button button--quiet migration-close" onClick={() => setOpen(false)}>{t("Close")}</button>}
    {running && <div className="migration-overlay" role="dialog" aria-modal="true" aria-labelledby="migration-progress-title"><div className="migration-overlay__panel"><span className="migration-overlay__icon">{progress?.stage === "activating" ? <Server /> : <DatabaseBackup />}</span><div className="eyebrow">{t("Library migration")}</div><h2 id="migration-progress-title">{progress?.stage === "activating" ? t("Making the NAS primary…") : t("Moving your library safely…")}</h2><p>{progress?.message ?? t("Preparing the migration…")}</p><div className="migration-progress"><i style={{ width: `${percent}%` }} /></div><div className="migration-progress__meta"><span>{percent}%</span><span>{progress?.totalBytes ? `${formatBytes(progress.completedBytes)} / ${formatBytes(progress.totalBytes)}` : t("Preparing")}</span></div><small>{t("Keep Volum open. If the connection drops, you can resume without starting over.")}</small></div></div>}
    {!running && progress?.stage === "activating" && !error && <div className="migration-complete"><Check size={17} />{t("The NAS is now the canonical library.")}</div>}
  </section>;
}
