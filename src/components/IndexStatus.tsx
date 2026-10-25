import { useEffect, useState } from "react";
import { CheckCircle2, LoaderCircle, Pause, Play } from "lucide-react";
import type { LibraryRoot, ScanStatus } from "../types";
import { api, onBackendEvent } from "../lib/tauri/api";
import { plural, t } from "../lib/i18n";

export function IndexStatus({ roots }: { roots: LibraryRoot[] }) {
  const [statuses, setStatuses] = useState<Record<string, ScanStatus>>({});
  const rootIds = roots.map((root) => root.id).join("|");
  useEffect(() => {
    if (!roots.length) return;
    void Promise.all(roots.map((root) => api.scanStatus(root.id))).then((values) => {
      setStatuses(Object.fromEntries(values.map((status) => [status.rootId, status])));
    }).catch(() => undefined);
    let unlisten: () => void = () => undefined;
    onBackendEvent<ScanStatus>("scan-progress", (status) => {
      setStatuses((current) => ({ ...current, [status.rootId]: status }));
    }).then((fn) => { unlisten = fn; });
    return () => unlisten();
  }, [rootIds]);
  const status = roots.map((root) => statuses[root.id]).find((item) => item?.state === "scanning" || item?.state === "paused" || item?.state === "error") ?? roots.map((root) => statuses[root.id]).find(Boolean);
  if (!status || status.state === "idle") return null;
  if (status.state === "complete") return <div className="index-status index-status--complete"><CheckCircle2 size={15} /><span>{t("Library up to date")}</span><small>{plural(status.processed, "{count} project", "{count} projects")}</small></div>;
  const percent = status.discovered ? Math.round(status.processed / status.discovered * 100) : 0;
  return <div className="index-status"><LoaderCircle size={15} className={status.state === "scanning" ? "spin" : ""} /><span>{t(status.state === "paused" ? "Indexing paused" : "Building previews")}</span><div className="index-status__progress"><i style={{ width: `${percent}%` }} /></div><small>{status.processed} / {status.discovered}</small><button className="icon-button icon-button--tiny" onClick={() => status.state === "paused" ? api.startScan(status.rootId) : api.pauseScan(status.rootId)}>{status.state === "paused" ? <Play size={13} /> : <Pause size={13} />}</button></div>;
}
