import { useEffect, useState } from "react";
import { CheckCircle2, LoaderCircle, Pause, Play } from "lucide-react";
import type { LibraryRoot, ScanStatus } from "../types";
import { api, onBackendEvent } from "../lib/tauri/api";
import { plural, t } from "../lib/i18n";

export function IndexStatus({ roots }: { roots: LibraryRoot[] }) {
  const [status, setStatus] = useState<ScanStatus>();
  useEffect(() => {
    if (!roots[0]) return;
    const refresh = () => api.scanStatus(roots[0].id).then(setStatus).catch(() => undefined);
    refresh();
    let unlisten: () => void = () => undefined;
    onBackendEvent("scan-progress", refresh).then((fn) => { unlisten = fn; });
    return () => unlisten();
  }, [roots]);
  if (!status || status.state === "idle") return null;
  if (status.state === "complete") return <div className="index-status index-status--complete"><CheckCircle2 size={15} /><span>{t("Library up to date")}</span><small>{plural(status.processed, "{count} model", "{count} models")}</small></div>;
  const percent = status.discovered ? Math.round(status.processed / status.discovered * 100) : 0;
  return <div className="index-status"><LoaderCircle size={15} className={status.state === "scanning" ? "spin" : ""} /><span>{t(status.state === "paused" ? "Indexing paused" : "Building previews")}</span><div className="index-status__progress"><i style={{ width: `${percent}%` }} /></div><small>{status.processed} / {status.discovered}</small><button className="icon-button icon-button--tiny" onClick={() => status.state === "paused" ? api.startScan(status.rootId) : api.pauseScan(status.rootId)}>{status.state === "paused" ? <Play size={13} /> : <Pause size={13} />}</button></div>;
}
