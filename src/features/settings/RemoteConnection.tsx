import { useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { Cable, Check, DatabaseBackup, ExternalLink, FolderOpen, HardDrive, ListChecks, LoaderCircle, RefreshCw, Server, Trash2 } from "lucide-react";
import { open } from "@tauri-apps/plugin-dialog";
import { api, configureRemoteMirror, connectRemote, forgetRemoteServer, initializeBackend, isTauri, listMirroredAssets, pendingLibraryMigration, rollbackLibraryMigration, syncRemoteMirror, useLocalBackend, useRemoteBackend } from "../../lib/tauri/api";
import type { BackendConnectionStatus } from "../../types";
import { formatBytes } from "../../lib/format";
import { useRemoteMirror } from "../mirror/useRemoteMirror";
import { t } from "../../lib/i18n";
import { LibraryMigration } from "./LibraryMigration";

interface Props {
  status: BackendConnectionStatus;
  onChange: (status: BackendConnectionStatus) => void;
  compact?: boolean;
}

export function RemoteConnection({ status, onChange, compact = false }: Props) {
  const [payload, setPayload] = useState("");
  const [busy, setBusy] = useState("");
  const [error, setError] = useState("");
  const [syncMessage, setSyncMessage] = useState("");
  const [setupMode, setSetupMode] = useState<"all" | "selected">("selected");
  const mirror = useRemoteMirror();
  const pendingMigration = useQuery({ queryKey: ["pending-library-migration"], queryFn: pendingLibraryMigration, retry: false });
  const act = async (name: string, action: () => Promise<BackendConnectionStatus>) => {
    setBusy(name); setError("");
    try { onChange(await action()); }
    catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)); }
    finally { setBusy(""); }
  };
  const mirrorAction = async (name: string, action: () => Promise<void>) => {
    setBusy(name); setError("");
    try { await action(); }
    catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)); }
    finally { setBusy(""); }
  };
  const rollback = async () => {
    setBusy("rollback"); setError("");
    try {
      onChange(await rollbackLibraryMigration(true));
      const { relaunch } = await import("@tauri-apps/plugin-process");
      await relaunch();
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
      setBusy("");
    }
  };
  const sync = async () => {
    const report = await syncRemoteMirror();
    setSyncMessage(t("Synced {total} files · {downloaded} downloaded · {uploaded} uploaded{conflicts}", {
      total: report.totalAssets,
      downloaded: report.downloaded,
      uploaded: report.uploaded,
      conflicts: report.conflicts ? t(" · {count} conflicts preserved", { count: report.conflicts }) : "",
    }));
    onChange(await initializeBackend());
    await mirror.refresh();
  };
  const chooseMirrorFolder = async () => {
    const path = isTauri() ? await open({ directory: true, multiple: false, title: t("Choose a folder for local project files") }) : "/Users/you/Volum Mirror";
    if (!path || Array.isArray(path)) return;
    onChange(await configureRemoteMirror(path, true, setupMode));
    await mirror.refresh();
    if (setupMode === "all") await sync();
  };
  const mirrorSettings = mirror.settings;
  const selectedRuleCount = (mirrorSettings?.folderIds.length ?? 0) + (mirrorSettings?.collectionIds.length ?? 0);
  return (
    <div className={`remote-connection ${compact ? "remote-connection--compact" : ""}`}>
      <div className="remote-connection__summary">
        <span className={`root-icon root-icon--${status.mode === "local" || status.online ? "online" : "offline"}`}>{status.mode === "local" ? <HardDrive size={20} /> : <Server size={20} />}</span>
        <div><strong>{status.mode === "local" ? "This Desktop" : status.serverName ?? "Remote Volum server"}</strong><small>{status.mode === "local" ? "Using the library stored on this computer" : status.baseUrl}</small></div>
        <span className={`status-pill status-pill--${status.mode === "local" || status.online ? "online" : "offline"}`}>{status.mode === "local" ? "Local" : status.online ? "Connected" : "Offline"}</span>
      </div>
      {status.mode === "remote" && !status.online && <div className="remote-connection__offline"><p>{status.lastError ?? "The server cannot be reached. Your mirrored files remain available offline."}</p><div className="settings-actions"><button className="button button--quiet" disabled={Boolean(busy)} onClick={() => void act("retry", initializeBackend)}>{busy === "retry" ? <LoaderCircle className="spin" size={15} /> : <RefreshCw size={15} />}Retry</button>{!status.localLibraryRetired && <button className="button button--quiet" onClick={() => void act("local", useLocalBackend)}><HardDrive size={15} />Use local library</button>}</div>{status.localLibraryRetired && <p className="settings-footnote">{t("This Desktop library is preserved as a read-only migration rollback. Reconnect to the NAS instead of creating a second canonical library.")}</p>}</div>}
      {status.mode === "local" && status.configured && <div className="settings-actions"><button className="button button--quiet" disabled={Boolean(busy)} onClick={() => void act("remote", useRemoteBackend)}><Server size={15} />Use {status.serverName ?? "remote server"}</button><button className="button button--quiet" disabled={Boolean(busy)} onClick={() => void act("forget", forgetRemoteServer)}><Trash2 size={15} />Forget server</button></div>}
      {status.mode === "remote" && status.online && <div className="remote-connection__mirror">
        <div><strong>{t("Local project files")}</strong><small>{status.mirrorEnabled ? status.mirrorPath : t("Keep original project files on this Desktop for offline and slicer access.")}</small></div>
        {!status.mirrorEnabled && <div className="mirror-mode-choices" role="radiogroup" aria-label={t("Local storage mode")}>
          <button className={setupMode === "selected" ? "is-active" : ""} role="radio" aria-checked={setupMode === "selected"} onClick={() => setSetupMode("selected")}><ListChecks size={18} /><span><strong>{t("Selected projects")}</strong><small>{t("Choose projects, folders, or collections.")}</small></span>{setupMode === "selected" && <Check size={15} />}</button>
          <button className={setupMode === "all" ? "is-active" : ""} role="radio" aria-checked={setupMode === "all"} onClick={() => setSetupMode("all")}><HardDrive size={18} /><span><strong>{t("Entire library")}</strong><small>{t("Keep every original file on this Desktop.")}</small></span>{setupMode === "all" && <Check size={15} />}</button>
        </div>}
        {status.mirrorEnabled && <>
          <div className="mirror-mode-segmented" role="group" aria-label={t("Local storage mode")}>
            <button className={mirrorSettings?.mode === "selected" ? "is-active" : ""} disabled={Boolean(busy) || mirror.busy} onClick={() => void mirrorAction("mode", async () => { await mirror.changeMode("selected"); onChange(await initializeBackend()); })}>{t("Selected projects")}</button>
            <button className={mirrorSettings?.mode === "all" ? "is-active" : ""} disabled={Boolean(busy) || mirror.busy} onClick={() => void mirrorAction("mode", async () => { await mirror.changeMode("all"); onChange(await initializeBackend()); })}>{t("Entire library")}</button>
          </div>
          <p className="settings-footnote">{mirrorSettings?.mode === "selected"
            ? t("{projects} individually selected · {rules} folder or collection rules · {available} available locally", { projects: mirrorSettings.projectIds.length, rules: selectedRuleCount, available: mirror.projects.filter((project) => project.assetCount === project.availableAssetCount).length })
            : t("Every project is kept locally and updated automatically.")}</p>
        </>}
        <div className="settings-actions">{status.mirrorEnabled ? <><button className="button button--quiet" disabled={Boolean(busy) || mirror.busy} onClick={() => void mirrorAction("sync", sync)}>{busy === "sync" ? <LoaderCircle className="spin" size={15} /> : <RefreshCw size={15} />}{t("Sync now")}</button><button className="button button--quiet" disabled={Boolean(busy) || mirror.busy} onClick={() => void act("mirror-off", () => configureRemoteMirror(undefined, false))}>{t("Disable")}</button></> : <button className="button button--quiet" disabled={Boolean(busy)} onClick={() => void mirrorAction("mirror", chooseMirrorFolder)}><HardDrive size={15} />{t("Choose local folder")}</button>}</div>
        {syncMessage && <p className="settings-footnote">{syncMessage}</p>}{status.lastError && status.mirrorEnabled && <div className="form-error">{status.lastError}</div>}
      </div>}
      {status.localLibraryRetired && <div className="migration-rollback"><DatabaseBackup size={18} /><div><strong>{t("Desktop rollback copy")}</strong><small>{t("The former local library is frozen. Restore it only after stopping volumd; NAS changes made after migration are not merged back.")}</small></div><button className="button button--quiet" disabled={Boolean(busy)} onClick={() => { if (window.confirm(t("Stop volumd first. Restore the old Desktop library and abandon any later NAS changes?"))) void rollback(); }}>{busy === "rollback" ? <LoaderCircle className="spin" size={15} /> : null}{t("Restore local copy")}</button></div>}
      {!pendingMigration.isLoading && !pendingMigration.data && <div className="remote-connection__pair">
        <label className="field"><span>{status.configured ? "Pair another server" : "Connect to a server"}</span><textarea rows={compact ? 3 : 5} value={payload} onChange={(event) => setPayload(event.target.value)} placeholder='Paste the pairing payload shown by Volum Desktop or volumd' /></label>
        <button className="button button--primary" disabled={!payload.trim() || Boolean(busy)} onClick={() => void act("pair", async () => { const next = await connectRemote(payload); setPayload(""); return next; })}>{busy === "pair" ? <LoaderCircle className="spin" size={15} /> : <Cable size={15} />}Pair and connect</button>
      </div>}
      {status.mode === "local" && !status.localLibraryRetired && <LibraryMigration onComplete={onChange} />}
      {error && <div className="form-error" role="alert">{error}</div>}
    </div>
  );
}

export function RemoteOfflineScreen({ status, onChange }: Omit<Props, "compact">) {
  const { data: mirrored = [] } = useQuery({ queryKey: ["mirrored-assets", status.serverId], queryFn: listMirroredAssets, enabled: status.mirrorEnabled, retry: false });
  const groups = useMemo(() => {
    const values = new Map<string, { name: string; assets: typeof mirrored }>();
    for (const asset of mirrored) {
      const group = values.get(asset.projectId) ?? { name: asset.projectName, assets: [] };
      group.assets.push(asset); values.set(asset.projectId, group);
    }
    return [...values.entries()];
  }, [mirrored]);
  return <div className="remote-offline-layout"><div className="onboarding remote-offline-screen"><div className="onboarding__panel"><div className="onboarding__art"><Server size={72} /></div></div><div className="onboarding__copy"><div className="eyebrow">{t("Remote library")}</div><h1>{t("{name} is offline.", { name: status.serverName ?? t("Your Volum server") })}</h1><p>{t(status.localLibraryRetired ? "Reconnect to browse the canonical NAS library. The former Desktop library remains frozen as a rollback copy." : "Reconnect to browse the canonical library, or switch back to this Desktop’s local library. Verified local originals remain available below.")}</p><RemoteConnection status={status} onChange={onChange} compact /></div></div>{mirrored.length > 0 && <section className="offline-mirror"><header><div><div className="eyebrow">{t("Local project files")}</div><h2>{t("Available original files")}</h2></div><span>{t("{projects} projects · {files} files", { projects: groups.length, files: mirrored.length })}</span></header><div className="offline-mirror__projects">{groups.map(([projectId, group]) => <section key={projectId}><header><strong>{group.name}</strong><small>{t("{count} files · {size}", { count: group.assets.length, size: formatBytes(group.assets.reduce((total, asset) => total + asset.byteSize, 0)) })}</small></header><div className="offline-mirror__list">{group.assets.map((asset) => <article key={asset.id}><span className="file-icon">{asset.filename.split(".").pop()?.toUpperCase()}</span><div><strong>{asset.filename}</strong><small>{formatBytes(asset.byteSize)}</small></div><button className="button button--quiet" onClick={() => void api.openAsset(asset.id)}><ExternalLink size={14} />{t("Open")}</button><button className="icon-button" onClick={() => void api.revealAsset(asset.id)} aria-label={t("Reveal local file")}><FolderOpen size={15} /></button></article>)}</div></section>)}</div></section>}</div>;
}
