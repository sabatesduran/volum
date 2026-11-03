import { useEffect, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { open } from "@tauri-apps/plugin-dialog";
import { FileArchive, FolderOpen, LoaderCircle, PackageOpen } from "lucide-react";
import { useAppStore } from "../app/store";
import { formatBytes, formatDate } from "../lib/format";
import { t } from "../lib/i18n";
import { api, isTauri } from "../lib/tauri/api";
import type { ProjectPackagePreview } from "../types";
import { Dialog } from "./Dialog";

export function ImportProjectPackageDialog({ onClose }: { onClose: () => void }) {
  const queryClient = useQueryClient();
  const selectModel = useAppStore((state) => state.selectModel);
  const { data: roots = [] } = useQuery({ queryKey: ["roots"], queryFn: api.roots });
  const [path, setPath] = useState("");
  const [preview, setPreview] = useState<ProjectPackagePreview>();
  const [rootId, setRootId] = useState("");
  const [relativeFolder, setRelativeFolder] = useState("");
  const [loading, setLoading] = useState(false);
  const [importing, setImporting] = useState(false);
  const [error, setError] = useState("");
  const selectedRoot = roots.find((root) => root.id === rootId);
  useEffect(() => { if (!rootId && roots.length) setRootId(roots.find((root) => root.status !== "offline")?.id ?? roots[0].id); }, [rootId, roots]);
  const choose = async () => {
    if (!isTauri()) return;
    const selected = await open({ multiple: false, filters: [{ name: t("Volum project package"), extensions: ["volumproject"] }] });
    if (!selected) return;
    setPath(selected); setPreview(undefined); setLoading(true); setError("");
    try {
      const next = await api.inspectProjectPackage(selected);
      setPreview(next);
      setRelativeFolder(`Volum Imports/${next.projectName.replace(/[\\/:*?"<>|]/g, "_")}`);
    } catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)); }
    finally { setLoading(false); }
  };
  const runImport = async () => {
    if (!path || !rootId || (selectedRoot?.rootKind !== "managed" && !relativeFolder.trim())) return;
    setImporting(true); setError("");
    try {
      const projectId = await api.importProjectPackage({ path, rootId, relativeFolder: relativeFolder.trim() });
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["models"] }),
        queryClient.invalidateQueries({ queryKey: ["folders"] }),
        queryClient.invalidateQueries({ queryKey: ["collections"] }),
        queryClient.invalidateQueries({ queryKey: ["custom-fields"] })
      ]);
      onClose(); selectModel(projectId);
    } catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)); }
    finally { setImporting(false); }
  };
  return <Dialog title={t("Import project package")} subtitle={t("Validate a complete portable project before copying it into a library.")} onClose={onClose} size="medium">
    <div className="dialog-form package-import">
      <button className="package-picker" onClick={() => void choose()} disabled={loading || importing}><span><FileArchive size={22} /></span><div><strong>{path ? path.split(/[\\/]/).pop() : t("Choose a .volumproject file")}</strong><small>{t("Checksums and paths are verified before import")}</small></div><FolderOpen size={16} /></button>
      {loading && <div className="package-import__loading"><LoaderCircle className="spin" size={18} />{t("Validating package…")}</div>}
      {error && <p className="form-error">{error}</p>}
      {preview && <div className="package-preview"><div className="package-preview__mark"><PackageOpen size={24} /></div><div><h3>{preview.projectName}</h3><p>{t("Exported {date} with Volum {version}", { date: formatDate(preview.exportedAt), version: preview.appVersion })}</p><div className="package-preview__facts"><span>{t("{count} files", { count: preview.fileCount })}</span><span>{t("{count} photos", { count: preview.photoCount })}</span><span>{t("{count} BOM items", { count: preview.bomItemCount })}</span><span>{t("{count} steps", { count: preview.assemblyStepCount })}</span><span>{formatBytes(preview.totalBytes)}</span></div>{preview.alreadyImported && <strong className="package-preview__warning">{t("This project was imported before. A new copy will be created without overwriting it.")}</strong>}</div></div>}
       {preview && <><label className="field"><span>{t("Destination library")}</span><select value={rootId} onChange={(event) => setRootId(event.target.value)}>{roots.map((root) => <option value={root.id} key={root.id} disabled={root.status === "offline"}>{root.displayName}{root.rootKind === "managed" ? ` · ${t("Managed")}` : ` · ${t("Linked")}`}{root.status === "offline" ? ` · ${t("Offline")}` : ""}</option>)}</select></label>{selectedRoot?.rootKind === "managed" ? <div className="managed-destination is-ready"><span><FolderOpen size={18} /></span><div><strong>{t("Managed project folder")}</strong><small>{t("Volum will create a collision-safe folder under Projects and preserve the package metadata.")}</small></div></div> : <label className="field"><span>{t("New folder inside library")}</span><input value={relativeFolder} onChange={(event) => setRelativeFolder(event.target.value)} /><small>{t("Volum always creates a new folder and never overwrites existing files.")}</small></label>}</>}
       <div className="dialog__actions"><button className="button button--quiet" onClick={onClose}>{t("Cancel")}</button><button className="button button--primary" disabled={!preview || !rootId || (selectedRoot?.rootKind !== "managed" && !relativeFolder.trim()) || importing} onClick={() => void runImport()}>{importing ? <LoaderCircle className="spin" size={15} /> : <PackageOpen size={15} />}{t(importing ? "Importing…" : "Import project")}</button></div>
    </div>
  </Dialog>;
}
