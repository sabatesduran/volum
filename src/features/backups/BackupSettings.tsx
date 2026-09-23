import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { open } from "@tauri-apps/plugin-dialog";
import {
  ArchiveRestore, CheckCircle2, Cloud, FolderOpen, HardDrive, LoaderCircle, MoreHorizontal,
  Pencil, Plus, RefreshCw, ShieldCheck, Trash2, TriangleAlert
} from "lucide-react";
import { Dialog } from "../../components/Dialog";
import { api, isTauri } from "../../lib/tauri/api";
import { formatBytes, formatDate } from "../../lib/format";
import { plural, t } from "../../lib/i18n";
import type { BackupArchive, BackupDestination, BackupDestinationInput, BackupSchedule, PreparedRestore } from "../../types";

const emptyDestination: BackupDestinationInput = {
  name: "",
  kind: "folder",
  location: "",
  schedule: "weekly",
  retentionCount: 7,
  enabled: true
};

function scheduleLabel(schedule: BackupSchedule) {
  return t(schedule === "manual" ? "Manual only" : schedule === "daily" ? "Daily" : schedule === "weekly" ? "Weekly" : "Monthly");
}

function DestinationDialog({ destination, onClose, onSaved }: { destination?: BackupDestination; onClose: () => void; onSaved: () => void }) {
  const [input, setInput] = useState<BackupDestinationInput>(destination ? {
    id: destination.id,
    name: destination.name,
    kind: destination.kind,
    location: destination.location,
    username: destination.username,
    schedule: destination.schedule,
    retentionCount: destination.retentionCount,
    enabled: destination.enabled
  } : emptyDestination);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState("");
  const chooseFolder = async () => {
    const path = isTauri() ? await open({ directory: true, multiple: false, title: t("Choose a backup folder") }) : "/Volumes/Home NAS/Volum Backups";
    if (path && !Array.isArray(path)) setInput((current) => ({ ...current, location: path, name: current.name || t("Backup folder") }));
  };
  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    setSaving(true);
    setError("");
    try {
      await api.saveBackupDestination(input);
      onSaved();
      onClose();
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setSaving(false);
    }
  };
  return <Dialog title={t(destination ? "Edit backup destination" : "Add backup destination")} subtitle={t("Backups contain Volum data and user media, but never your original model files.")} onClose={onClose} size="medium">
    <form className="dialog-form backup-destination-form" onSubmit={submit}>
      <fieldset><legend>{t("Destination type")}</legend><div className="collection-type-choice"><button type="button" className={input.kind === "folder" ? "is-active" : ""} onClick={() => setInput((current) => ({ ...current, kind: "folder" }))}><HardDrive size={17} /><span><strong>{t("Folder")}</strong><small>{t("Local, external, SMB or NFS")}</small></span></button><button type="button" className={input.kind === "webdav" ? "is-active" : ""} onClick={() => setInput((current) => ({ ...current, kind: "webdav" }))}><Cloud size={17} /><span><strong>WebDAV</strong><small>{t("Connect to a NAS directly")}</small></span></button></div></fieldset>
      <label className="field"><span>{t("Name")}</span><input autoFocus value={input.name} maxLength={100} required onChange={(event) => setInput((current) => ({ ...current, name: event.target.value }))} placeholder={input.kind === "folder" ? t("Home NAS") : t("NAS WebDAV")} /></label>
      {input.kind === "folder" ? <label className="field"><span>{t("Folder")}</span><div className="backup-path-field"><input value={input.location} required onChange={(event) => setInput((current) => ({ ...current, location: event.target.value }))} placeholder="/Volumes/NAS/Volum Backups" /><button type="button" className="button button--quiet" onClick={() => void chooseFolder()}><FolderOpen size={14} /> {t("Choose")}</button></div><small>{t("Mounted network shares and synced folders work like normal folders.")}</small></label> : <>
        <label className="field"><span>{t("WebDAV folder URL")}</span><input type="url" value={input.location} required onChange={(event) => setInput((current) => ({ ...current, location: event.target.value }))} placeholder="https://nas.example.com/dav/Volum/" /></label>
        <div className="calculator-grid"><label className="field field--compact"><span>{t("Username")}</span><input value={input.username ?? ""} onChange={(event) => setInput((current) => ({ ...current, username: event.target.value }))} autoComplete="username" /></label><label className="field field--compact"><span>{t("Password")}</span><input type="password" value={input.password ?? ""} onChange={(event) => setInput((current) => ({ ...current, password: event.target.value }))} autoComplete="current-password" placeholder={destination?.credentialSaved ? t("Leave blank to keep") : ""} /></label></div>
        <small className="form-hint">{t("The password is stored in your operating system’s credential manager, never in Volum’s database or backups.")}</small>
      </>}
      <div className="calculator-grid"><label className="field field--compact"><span>{t("Schedule")}</span><select value={input.schedule} onChange={(event) => setInput((current) => ({ ...current, schedule: event.target.value as BackupSchedule }))}><option value="manual">{t("Manual only")}</option><option value="daily">{t("Daily")}</option><option value="weekly">{t("Weekly")}</option><option value="monthly">{t("Monthly")}</option></select></label><label className="field field--compact"><span>{t("Backups to keep")}</span><input type="number" min={1} max={1000} value={input.retentionCount} onChange={(event) => setInput((current) => ({ ...current, retentionCount: Math.max(1, Number(event.target.value)) }))} /></label></div>
      <label className="backup-enabled"><input type="checkbox" checked={input.enabled} onChange={(event) => setInput((current) => ({ ...current, enabled: event.target.checked }))} /><span><strong>{t("Enable this destination")}</strong><small>{t("Disabled destinations remain configured but do not run automatically.")}</small></span></label>
      {error && <div className="form-error">{error}</div>}
      <div className="dialog__actions"><button type="button" className="button button--quiet" onClick={onClose}>{t("Cancel")}</button><button className="button button--primary" disabled={saving || !input.name.trim() || !input.location.trim()}>{saving ? <LoaderCircle className="spin" size={14} /> : null}{t(saving ? "Saving…" : "Save destination")}</button></div>
    </form>
  </Dialog>;
}

export function RestoreBackupDialog({ prepared, onClose }: { prepared: PreparedRestore; onClose: () => void }) {
  const [restoring, setRestoring] = useState(false);
  const [error, setError] = useState("");
  const cancel = async () => {
    await api.cancelPreparedRestore(prepared.token).catch(() => undefined);
    onClose();
  };
  const restore = async () => {
    setRestoring(true);
    setError("");
    try {
      await api.commitPreparedRestore(prepared.token);
      if (isTauri()) {
        const { relaunch } = await import("@tauri-apps/plugin-process");
        await relaunch();
      } else onClose();
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
      setRestoring(false);
    }
  };
  return <Dialog title={t("Restore this backup?")} subtitle={t("Volum validated the archive and will make a safety backup of the current app data first.")} onClose={() => { if (!restoring) void cancel(); }} size="medium">
    <div className="restore-summary"><span className="restore-summary__icon"><ArchiveRestore size={23} /></span><div><strong>{t("Volum {version}", { version: prepared.appVersion })}</strong><small>{t("Created {date}", { date: formatDate(prepared.createdAt) })} · {formatBytes(prepared.byteSize)}</small></div></div>
    <div className="restore-counts"><div><strong>{prepared.counts.models.toLocaleString()}</strong><span>{t("Projects")}</span></div><div><strong>{prepared.counts.collections.toLocaleString()}</strong><span>{t("Collections")}</span></div><div><strong>{prepared.counts.tags.toLocaleString()}</strong><span>{t("Tags")}</span></div><div><strong>{prepared.counts.webSources.toLocaleString()}</strong><span>{t("Web imports")}</span></div></div>
    <div className="backup-warning"><TriangleAlert size={17} /><span><strong>{t("This replaces the current Volum library data.")}</strong><small>{t("Original model files are never changed. Libraries at different paths can be reconnected after restart.")}</small></span></div>
    {error && <div className="form-error">{error}</div>}
    <div className="dialog__actions"><button className="button button--quiet" disabled={restoring} onClick={() => void cancel()}>{t("Cancel")}</button><button className="button button--primary" disabled={restoring} onClick={() => void restore()}>{restoring ? <LoaderCircle className="spin" size={14} /> : <ArchiveRestore size={14} />}{t(restoring ? "Preparing restart…" : "Restore & restart")}</button></div>
  </Dialog>;
}

function ArchiveDialog({ destination, archives, loading, error, onChoose, onClose }: { destination: BackupDestination; archives: BackupArchive[]; loading: boolean; error?: string; onChoose: (archive: BackupArchive) => void; onClose: () => void }) {
  return <Dialog title={t("Backups on {name}", { name: destination.name })} subtitle={t("Choose a backup to validate before restoring it.")} onClose={onClose} size="medium"><div className="backup-archive-list">{loading ? <div className="backup-loading"><LoaderCircle className="spin" size={17} /> {t("Loading backups…")}</div> : error ? <div className="backup-empty backup-destination__error">{error}</div> : archives.length ? archives.map((archive) => <button key={archive.key} onClick={() => onChoose(archive)}><span className="backup-archive-icon"><ArchiveRestore size={16} /></span><span><strong>{archive.filename}</strong><small>{archive.modifiedAt ? formatDate(archive.modifiedAt) : t("Volum backup")}{archive.byteSize ? ` · ${formatBytes(archive.byteSize)}` : ""}</small></span></button>) : <div className="backup-empty">{t("No Volum backups found at this destination.")}</div>}</div><div className="dialog__actions"><button className="button button--quiet" onClick={onClose}>{t("Close")}</button></div></Dialog>;
}

export function BackupSettings() {
  const queryClient = useQueryClient();
  const { data: destinations = [] } = useQuery({ queryKey: ["backup-destinations"], queryFn: api.backupDestinations });
  const { data: runs = [] } = useQuery({ queryKey: ["backup-runs"], queryFn: () => api.backupRuns(8) });
  const [editing, setEditing] = useState<BackupDestination | "new">();
  const [busy, setBusy] = useState("");
  const [message, setMessage] = useState<{ kind: "success" | "error"; text: string }>();
  const [archiveDestination, setArchiveDestination] = useState<BackupDestination>();
  const [prepared, setPrepared] = useState<PreparedRestore>();
  const archivesQuery = useQuery({ queryKey: ["backup-archives", archiveDestination?.id], queryFn: () => api.backupArchives(archiveDestination!.id), enabled: Boolean(archiveDestination) });
  const refresh = async () => Promise.all([queryClient.invalidateQueries({ queryKey: ["backup-destinations"] }), queryClient.invalidateQueries({ queryKey: ["backup-runs"] }), queryClient.invalidateQueries({ queryKey: ["backup-archives"] })]);
  const run = async (destination: BackupDestination) => {
    setBusy(`run:${destination.id}`); setMessage(undefined);
    try {
      const [result] = await api.runBackup(destination.id);
      if (result?.status === "failed") throw new Error(result.error || t("Backup failed"));
      setMessage({ kind: "success", text: t("Backup saved to {name}.", { name: destination.name }) });
    } catch (reason) { setMessage({ kind: "error", text: reason instanceof Error ? reason.message : String(reason) }); }
    finally { setBusy(""); await refresh(); }
  };
  const test = async (destination: BackupDestination) => {
    setBusy(`test:${destination.id}`); setMessage(undefined);
    try { await api.testBackupDestination(destination.id); setMessage({ kind: "success", text: t("Connected to {name}.", { name: destination.name }) }); }
    catch (reason) { setMessage({ kind: "error", text: reason instanceof Error ? reason.message : String(reason) }); }
    finally { setBusy(""); }
  };
  const remove = async (destination: BackupDestination) => {
    if (!window.confirm(t("Remove “{name}” as a backup destination? Existing backup files will not be deleted.", { name: destination.name }))) return;
    setMessage(undefined);
    try { await api.deleteBackupDestination(destination.id); await refresh(); }
    catch (reason) { setMessage({ kind: "error", text: reason instanceof Error ? reason.message : String(reason) }); }
  };
  const restoreFile = async () => {
    const path = isTauri() ? await open({ multiple: false, directory: false, title: t("Choose a Volum backup"), filters: [{ name: t("Volum backup"), extensions: ["zip"] }] }) : "/Users/you/Backups/volum-backup-demo.zip";
    if (!path || Array.isArray(path)) return;
    setBusy("restore:file"); setMessage(undefined);
    try { setPrepared(await api.prepareRestoreFromPath(path)); }
    catch (reason) { setMessage({ kind: "error", text: reason instanceof Error ? reason.message : String(reason) }); }
    finally { setBusy(""); }
  };
  const restoreArchive = async (archive: BackupArchive) => {
    if (!archiveDestination) return;
    setBusy("restore:destination"); setMessage(undefined);
    try { const result = await api.prepareRestoreFromDestination(archiveDestination.id, archive.key); setArchiveDestination(undefined); setPrepared(result); }
    catch (reason) { setMessage({ kind: "error", text: reason instanceof Error ? reason.message : String(reason) }); }
    finally { setBusy(""); }
  };
  return <section className="settings-section" id="backups">
    <div className="settings-section__header"><div><h2>{t("Backups")}</h2><p>{t("Automatically protect Volum’s organization, settings, attribution, and user media.")}</p></div><button className="button button--primary" onClick={() => setEditing("new")}><Plus size={15} /> {t("Add destination")}</button></div>
    {message && <div className={`backup-message is-${message.kind}`}>{message.kind === "success" ? <CheckCircle2 size={16} /> : <TriangleAlert size={16} />}<span>{message.text}</span></div>}
    <div className="backup-destination-list">{destinations.map((destination) => <article className={`backup-destination ${!destination.enabled ? "is-disabled" : ""}`} key={destination.id}><span className={`backup-destination__icon is-${destination.kind}`}>{destination.kind === "folder" ? <HardDrive size={19} /> : <Cloud size={19} />}</span><div className="backup-destination__copy"><div><strong>{destination.name}</strong><span>{destination.kind === "folder" ? t("Folder") : "WebDAV"}</span>{!destination.enabled && <span>{t("Disabled")}</span>}{destination.kind === "webdav" && destination.username && !destination.credentialSaved && <span>{t("Password required")}</span>}</div><small>{destination.location}</small><small>{scheduleLabel(destination.schedule)} · {plural(destination.retentionCount, "Keep {count} backup", "Keep {count} backups")}{destination.lastSuccessAt ? ` · ${t("Last backup {date}", { date: formatDate(destination.lastSuccessAt) })}` : ""}</small>{destination.lastError && <small className="backup-destination__error">{destination.lastError}</small>}</div><div className="backup-destination__actions"><button className="button button--quiet" disabled={Boolean(busy)} onClick={() => void run(destination)}>{busy === `run:${destination.id}` ? <LoaderCircle className="spin" size={14} /> : <RefreshCw size={14} />}{t("Back up")}</button><button className="icon-button" disabled={Boolean(busy)} onClick={() => void test(destination)} title={t("Test connection")} aria-label={t("Test connection")}>{busy === `test:${destination.id}` ? <LoaderCircle className="spin" size={15} /> : <ShieldCheck size={15} />}</button><button className="icon-button" onClick={() => setArchiveDestination(destination)} title={t("Browse backups")} aria-label={t("Browse backups")}><ArchiveRestore size={15} /></button><button className="icon-button" onClick={() => setEditing(destination)} title={t("Edit")} aria-label={t("Edit")}><Pencil size={15} /></button><button className="icon-button" onClick={() => void remove(destination)} title={t("Remove")} aria-label={t("Remove")}><Trash2 size={15} /></button></div></article>)}{destinations.length === 0 && <div className="backup-onboarding"><span><HardDrive size={22} /></span><div><strong>{t("No backup destination yet")}</strong><p>{t("Choose a folder, mounted NAS, or WebDAV server. Automatic backups run whenever Volum is open and a schedule is due.")}</p></div></div>}</div>
    <div className="backup-footer"><button className="button button--quiet" disabled={Boolean(busy)} onClick={() => void restoreFile()}>{busy === "restore:file" ? <LoaderCircle className="spin" size={14} /> : <ArchiveRestore size={14} />}{t("Restore from ZIP…")}</button><small>{t("Backups do not contain original model files or regenerable preview thumbnails.")}</small></div>
    {runs.length > 0 && <details className="backup-history"><summary><MoreHorizontal size={15} />{t("Recent backup activity")}</summary><div>{runs.map((run) => <div key={run.id}><span className={`backup-run-status is-${run.status}`}>{run.status === "complete" ? <CheckCircle2 size={14} /> : run.status === "failed" ? <TriangleAlert size={14} /> : <LoaderCircle className="spin" size={14} />}</span><span><strong>{run.destinationName}</strong><small>{formatDate(run.startedAt)} · {run.reason === "scheduled" ? t("Scheduled") : t("Manual")}{run.byteSize ? ` · ${formatBytes(run.byteSize)}` : ""}</small>{run.error && <small className="backup-destination__error">{run.error}</small>}</span></div>)}</div></details>}
    {editing && <DestinationDialog destination={editing === "new" ? undefined : editing} onClose={() => setEditing(undefined)} onSaved={() => void refresh()} />}
    {archiveDestination && <ArchiveDialog destination={archiveDestination} archives={archivesQuery.data ?? []} loading={archivesQuery.isLoading || busy === "restore:destination"} error={archivesQuery.error instanceof Error ? archivesQuery.error.message : archivesQuery.error ? String(archivesQuery.error) : undefined} onChoose={(archive) => void restoreArchive(archive)} onClose={() => setArchiveDestination(undefined)} />}
    {prepared && <RestoreBackupDialog prepared={prepared} onClose={() => setPrepared(undefined)} />}
  </section>;
}
