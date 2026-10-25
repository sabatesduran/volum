import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { open } from "@tauri-apps/plugin-dialog";
import { Check, Copy, FolderInput, FolderOpen, HardDrive, Link2, LoaderCircle, Plus } from "lucide-react";
import { formatBytes } from "../lib/format";
import { t } from "../lib/i18n";
import { api, isTauri } from "../lib/tauri/api";
import type { ModelDetail } from "../types";
import { Dialog } from "./Dialog";

export function AddProjectFilesDialog({ model, onClose, onComplete }: { model: ModelDetail; onClose: () => void; onComplete: () => Promise<void> }) {
  const [paths, setPaths] = useState<string[]>([]);
  const [mode, setMode] = useState<"move" | "copy" | "link">(model.storageMode === "managed" ? "move" : "link");
  const [working, setWorking] = useState(false);
  const [error, setError] = useState("");
  const choose = async () => {
    const selected = isTauri() ? await open({ multiple: true, directory: false, title: t("Choose 3D project files"), filters: [{ name: t("3D files"), extensions: ["stl", "3mf", "obj", "step", "stp"] }] }) : ["/Users/you/Downloads/part.stl"];
    if (!selected) return;
    setPaths(Array.isArray(selected) ? selected : [selected]);
  };
  const add = async () => {
    if (!paths.length) return;
    setWorking(true); setError("");
    try {
      await api.addProjectFiles({ projectId: model.id, filePaths: paths, assetIds: [], transferMode: mode });
      await onComplete(); onClose();
    } catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)); }
    finally { setWorking(false); }
  };
  const choices = model.storageMode === "managed"
    ? [["move", FolderInput, "Move into project", "Recommended · Volum becomes the source of truth"], ["copy", Copy, "Copy into project", "Keep an original outside Volum"]] as const
    : [["link", Link2, "Keep linked", "Add the files without moving them"]] as const;
  return <Dialog title={t("Add project files")} subtitle={t(model.storageMode === "managed" ? "Files are placed inside this project’s managed folder." : "This linked project keeps its files in external folders.")} onClose={onClose}>
    <div className="dialog-form"><button className="package-picker" onClick={() => void choose()}><span><FolderOpen size={21} /></span><div><strong>{paths.length ? t("{count} files selected", { count: paths.length }) : t("Choose 3D files…")}</strong><small>{paths.length ? paths.map((path) => path.split(/[\\/]/).pop()).join(" · ") : t("STL, 3MF, OBJ, STEP, or STP")}</small></div><Plus size={16} /></button>
      <fieldset className="transfer-options transfer-options--compact"><legend>{t("File handling")}</legend>{choices.map(([value, Icon, title, description]) => <button type="button" key={value} className={mode === value ? "is-active" : ""} onClick={() => setMode(value)}><span><Icon size={17} /></span><strong>{t(title)}</strong><small>{t(description)}</small>{mode === value && <Check size={14} />}</button>)}</fieldset>
      {error && <p className="form-error">{error}</p>}<div className="dialog__actions"><button className="button button--quiet" onClick={onClose}>{t("Cancel")}</button><button className="button button--primary" disabled={!paths.length || working} onClick={() => void add()}>{working ? <LoaderCircle className="spin" size={15} /> : <Plus size={15} />}{t(working ? "Adding files…" : "Add files")}</button></div>
    </div>
  </Dialog>;
}

export function ConsolidateProjectDialog({ model, onClose, onComplete }: { model: ModelDetail; onClose: () => void; onComplete: () => Promise<void> }) {
  const queryClient = useQueryClient();
  const { data: preview, isLoading, error: previewError } = useQuery({ queryKey: ["consolidation-preview", model.id], queryFn: () => api.consolidationPreview(model.id) });
  const { data: managed } = useQuery({ queryKey: ["managed-library"], queryFn: api.managedLibrary });
  const [mode, setMode] = useState<"move" | "copy">("move");
  const [working, setWorking] = useState(false);
  const [error, setError] = useState("");
  const configure = async () => {
    const path = isTauri() ? await open({ directory: true, multiple: false, title: t("Choose the managed Volum library") }) : "/Users/you/Volum";
    if (!path || Array.isArray(path)) return;
    await api.configureManagedLibrary(path);
    await Promise.all([queryClient.invalidateQueries({ queryKey: ["managed-library"] }), queryClient.invalidateQueries({ queryKey: ["roots"] })]);
  };
  const run = async () => {
    setWorking(true); setError("");
    try { await api.convertProjectToManaged(model.id, mode); await onComplete(); onClose(); }
    catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)); }
    finally { setWorking(false); }
  };
  return <Dialog title={t("Organize in Volum")} subtitle={t("Preview the consolidation before changing any files.")} onClose={onClose} size="medium"><div className="dialog-form consolidation-dialog">
    {isLoading ? <div className="package-import__loading"><LoaderCircle className="spin" />{t("Inspecting project files…")}</div> : preview && <div className="consolidation-preview"><span><HardDrive size={22} /></span><div><strong>{preview.projectName}</strong><small>{t("{count} files · {size}", { count: preview.fileCount, size: formatBytes(preview.totalBytes) })}</small><p>{t("New folder: Projects/{name}", { name: preview.managedFolderName })}</p>{preview.sourceFolders.map((folder) => <code key={folder}>{folder}</code>)}</div></div>}
    {previewError && <p className="form-error">{previewError instanceof Error ? previewError.message : String(previewError)}</p>}
    {!managed?.root ? <div className="managed-destination"><span><FolderOpen size={18} /></span><div><strong>{t("Managed library required")}</strong><small>{t("Choose where Volum should organize project folders.")}</small></div><button className="button button--quiet" onClick={() => void configure()}>{t("Choose…")}</button></div> : <div className="managed-destination is-ready"><span><FolderOpen size={18} /></span><div><strong>{t("Destination")}</strong><small>{managed.projectsPath}/{preview?.managedFolderName}</small></div></div>}
    <fieldset className="transfer-options"><legend>{t("Consolidation method")}</legend><button type="button" className={mode === "move" ? "is-active is-recommended" : "is-recommended"} onClick={() => setMode("move")}><span><FolderInput size={18} /></span><strong>{t("Move files")}</strong><small>{t("Recommended · remove originals after verified copies")}</small>{mode === "move" && <Check size={14} />}</button><button type="button" className={mode === "copy" ? "is-active" : ""} onClick={() => setMode("copy")}><span><Copy size={18} /></span><strong>{t("Copy files")}</strong><small>{t("Keep originals and ignore unchanged source copies")}</small>{mode === "copy" && <Check size={14} />}</button></fieldset>
    <p className="settings-footnote">{t("Volum stages every file, verifies its checksum, and preserves project IDs and documentation before changing the index.")}</p>
    {error && <p className="form-error">{error}</p>}<div className="dialog__actions"><button className="button button--quiet" onClick={onClose}>{t("Cancel")}</button><button className="button button--primary" disabled={!preview || !managed?.root || working} onClick={() => void run()}>{working ? <LoaderCircle className="spin" size={15} /> : <FolderInput size={15} />}{t(working ? "Organizing…" : mode === "move" ? "Move into Volum" : "Copy into Volum")}</button></div>
  </div></Dialog>;
}
