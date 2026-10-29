import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { Cable, ExternalLink, FolderOpen, HardDrive, LoaderCircle, RefreshCw, Server, Trash2 } from "lucide-react";
import { open } from "@tauri-apps/plugin-dialog";
import { api, configureRemoteMirror, connectRemote, forgetRemoteServer, initializeBackend, isTauri, listMirroredAssets, syncRemoteMirror, useLocalBackend, useRemoteBackend } from "../../lib/tauri/api";
import type { BackendConnectionStatus } from "../../types";
import { formatBytes } from "../../lib/format";

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
  const act = async (name: string, action: () => Promise<BackendConnectionStatus>) => {
    setBusy(name); setError("");
    try { onChange(await action()); }
    catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)); }
    finally { setBusy(""); }
  };
  return (
    <div className={`remote-connection ${compact ? "remote-connection--compact" : ""}`}>
      <div className="remote-connection__summary">
        <span className={`root-icon root-icon--${status.mode === "local" || status.online ? "online" : "offline"}`}>{status.mode === "local" ? <HardDrive size={20} /> : <Server size={20} />}</span>
        <div><strong>{status.mode === "local" ? "This Desktop" : status.serverName ?? "Remote Volum server"}</strong><small>{status.mode === "local" ? "Using the library stored on this computer" : status.baseUrl}</small></div>
        <span className={`status-pill status-pill--${status.mode === "local" || status.online ? "online" : "offline"}`}>{status.mode === "local" ? "Local" : status.online ? "Connected" : "Offline"}</span>
      </div>
      {status.mode === "remote" && !status.online && <div className="remote-connection__offline"><p>{status.lastError ?? "The server cannot be reached. Your mirrored files remain available offline."}</p><div className="settings-actions"><button className="button button--quiet" disabled={Boolean(busy)} onClick={() => void act("retry", initializeBackend)}>{busy === "retry" ? <LoaderCircle className="spin" size={15} /> : <RefreshCw size={15} />}Retry</button><button className="button button--quiet" onClick={() => void act("local", useLocalBackend)}><HardDrive size={15} />Use local library</button></div></div>}
      {status.mode === "local" && status.configured && <div className="settings-actions"><button className="button button--quiet" disabled={Boolean(busy)} onClick={() => void act("remote", useRemoteBackend)}><Server size={15} />Use {status.serverName ?? "remote server"}</button><button className="button button--quiet" disabled={Boolean(busy)} onClick={() => void act("forget", forgetRemoteServer)}><Trash2 size={15} />Forget server</button></div>}
      {status.mode === "remote" && status.online && <div className="remote-connection__mirror"><div><strong>Managed local mirror</strong><small>{status.mirrorEnabled ? status.mirrorPath : "Optional offline copies of original project files"}</small></div><div className="settings-actions">{status.mirrorEnabled ? <><button className="button button--quiet" disabled={Boolean(busy)} onClick={() => void act("sync", async () => { const report = await syncRemoteMirror(); setSyncMessage(`Synced ${report.totalAssets} files · ${report.downloaded} downloaded · ${report.uploaded} uploaded${report.conflicts ? ` · ${report.conflicts} conflicts preserved` : ""}`); return initializeBackend(); })}>{busy === "sync" ? <LoaderCircle className="spin" size={15} /> : <RefreshCw size={15} />}Sync now</button><button className="button button--quiet" disabled={Boolean(busy)} onClick={() => void act("mirror-off", () => configureRemoteMirror(undefined, false))}>Disable</button></> : <button className="button button--quiet" disabled={Boolean(busy)} onClick={() => void act("mirror", async () => { const path = isTauri() ? await open({ directory: true, multiple: false, title: "Choose a folder for the Volum mirror" }) : "/Users/you/Volum Mirror"; if (!path || Array.isArray(path)) return status; return configureRemoteMirror(path, true); })}><HardDrive size={15} />Choose mirror folder</button>}</div>{syncMessage && <p className="settings-footnote">{syncMessage}</p>}{status.lastError && status.mirrorEnabled && <div className="form-error">{status.lastError}</div>}</div>}
      <div className="remote-connection__pair">
        <label className="field"><span>{status.configured ? "Pair another server" : "Connect to a server"}</span><textarea rows={compact ? 3 : 5} value={payload} onChange={(event) => setPayload(event.target.value)} placeholder='Paste the pairing payload shown by Volum Desktop or volumd' /></label>
        <button className="button button--primary" disabled={!payload.trim() || Boolean(busy)} onClick={() => void act("pair", async () => { const next = await connectRemote(payload); setPayload(""); return next; })}>{busy === "pair" ? <LoaderCircle className="spin" size={15} /> : <Cable size={15} />}Pair and connect</button>
      </div>
      {error && <div className="form-error" role="alert">{error}</div>}
    </div>
  );
}

export function RemoteOfflineScreen({ status, onChange }: Omit<Props, "compact">) {
  const { data: mirrored = [] } = useQuery({ queryKey: ["mirrored-assets", status.serverId], queryFn: listMirroredAssets, enabled: status.mirrorEnabled, retry: false });
  return <div className="remote-offline-layout"><div className="onboarding remote-offline-screen"><div className="onboarding__panel"><div className="onboarding__art"><Server size={72} /></div></div><div className="onboarding__copy"><div className="eyebrow">Remote library</div><h1>{status.serverName ?? "Your Volum server"} is offline.</h1><p>Reconnect to browse the canonical library, or switch back to this Desktop’s local library. Verified mirrored originals remain available below.</p><RemoteConnection status={status} onChange={onChange} compact /></div></div>{mirrored.length > 0 && <section className="offline-mirror"><header><div><div className="eyebrow">Offline mirror</div><h2>Available original files</h2></div><span>{mirrored.length} files</span></header><div className="offline-mirror__list">{mirrored.slice(0, 250).map((asset) => <article key={asset.id}><span className="file-icon">{asset.filename.split(".").pop()?.toUpperCase()}</span><div><strong>{asset.filename}</strong><small>{asset.projectName} · {formatBytes(asset.byteSize)}</small></div><button className="button button--quiet" onClick={() => void api.openAsset(asset.id)}><ExternalLink size={14} />Open</button><button className="icon-button" onClick={() => void api.revealAsset(asset.id)} aria-label="Reveal mirrored file"><FolderOpen size={15} /></button></article>)}</div></section>}</div>;
}
