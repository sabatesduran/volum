import { useEffect, useMemo, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { open } from "@tauri-apps/plugin-dialog";
import { Check, Copy, FileBox, FolderInput, FolderOpen, Link2, LoaderCircle, Plus } from "lucide-react";
import { useAppStore } from "../app/store";
import { formatBytes } from "../lib/format";
import { t } from "../lib/i18n";
import { api, isTauri } from "../lib/tauri/api";
import type { LibraryAssetCandidate } from "../types";
import { Dialog } from "./Dialog";

type TransferMode = "move" | "copy" | "link";
const EMPTY_LIBRARY_ASSETS: LibraryAssetCandidate[] = [];
const EMPTY_PROJECT_IDS: string[] = [];

export function NewProjectDialog({ onClose, projectIds = EMPTY_PROJECT_IDS }: { onClose: () => void; projectIds?: string[] }) {
  const queryClient = useQueryClient();
  const selectModel = useAppStore((state) => state.selectModel);
  const clearSelection = useAppStore((state) => state.clearModelSelection);
  const { data: managed, refetch: refetchManaged } = useQuery({ queryKey: ["managed-library"], queryFn: api.managedLibrary });
  const { data: loadedAssets } = useQuery({ queryKey: ["library-assets"], queryFn: api.libraryAssets });
  const assets = loadedAssets ?? EMPTY_LIBRARY_ASSETS;
  const suggestedAssets = useMemo(() => assets.filter((asset) => projectIds.includes(asset.projectId) && !asset.missing), [assets, projectIds]);
  const [name, setName] = useState("");
  const [filePaths, setFilePaths] = useState<string[]>([]);
  const [assetIds, setAssetIds] = useState<string[]>([]);
  const [mode, setMode] = useState<TransferMode>("move");
  const [showLibrary, setShowLibrary] = useState(projectIds.length > 0);
  const [search, setSearch] = useState("");
  const [working, setWorking] = useState(false);
  const [error, setError] = useState("");
  useEffect(() => {
    setAssetIds(suggestedAssets.map((asset) => asset.id));
    if (!name && projectIds.length === 1 && suggestedAssets[0]) setName(suggestedAssets[0].projectName);
  }, [projectIds.length, suggestedAssets]);
  const visibleAssets = assets.filter((asset) => !asset.missing && (!search.trim() || `${asset.projectName} ${asset.filename} ${asset.rootName}`.toLocaleLowerCase().includes(search.trim().toLocaleLowerCase()))).slice(0, 250);
  const sourceCount = filePaths.length + assetIds.length;
  const hasManagedSelection = assets.some((asset) => assetIds.includes(asset.id) && asset.storageMode === "managed");
  const chooseFiles = async () => {
    const selected = isTauri() ? await open({ multiple: true, directory: false, title: t("Choose 3D project files"), filters: [{ name: t("3D files"), extensions: ["stl", "3mf", "obj", "step", "stp"] }] }) : ["/Users/you/Downloads/model.stl"];
    if (!selected) return;
    const paths = Array.isArray(selected) ? selected : [selected];
    setFilePaths((current) => [...new Set([...current, ...paths])]);
    if (!name && paths[0]) setName(paths[0].split(/[\\/]/).pop()?.replace(/\.[^.]+$/, "") ?? "");
  };
  const configure = async () => {
    const path = isTauri() ? await open({ directory: true, multiple: false, title: t("Choose the managed Volum library") }) : "/Users/you/Volum";
    if (!path || Array.isArray(path)) return;
    setError("");
    try {
      await api.configureManagedLibrary(path);
      await Promise.all([refetchManaged(), queryClient.invalidateQueries({ queryKey: ["roots"] })]);
    } catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)); }
  };
  const create = async () => {
    if (!name.trim()) return;
    if (mode !== "link" && !managed?.root) {
      setError(t("Choose a managed library before creating this project."));
      return;
    }
    setWorking(true); setError("");
    try {
      const projectId = await api.createProject({ name: name.trim(), filePaths, assetIds, transferMode: mode });
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["models"] }),
        queryClient.invalidateQueries({ queryKey: ["folders"] }),
        queryClient.invalidateQueries({ queryKey: ["roots"] }),
        queryClient.invalidateQueries({ queryKey: ["library-assets"] })
      ]);
      clearSelection(); onClose(); selectModel(projectId);
    } catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)); }
    finally { setWorking(false); }
  };
  return <Dialog title={t("New project")} subtitle={t("Create an empty project or bring several 3D files together without losing their identity.")} onClose={onClose} size="medium">
    <div className="dialog-form project-create">
      <label className="field"><span>{t("Project name")}</span><input autoFocus value={name} maxLength={160} onChange={(event) => setName(event.target.value)} placeholder={t("e.g. Modular camera mount")} /></label>
      <div className="project-source-actions"><button className="button button--quiet" onClick={() => void chooseFiles()}><FolderOpen size={15} />{t("Choose files…")}</button><button className="button button--quiet" onClick={() => setShowLibrary((value) => !value)}><FileBox size={15} />{t("Choose from library")}</button><span>{sourceCount ? t("{count} files selected", { count: sourceCount }) : t("No files — create an empty project")}</span></div>
      {filePaths.length > 0 && <div className="project-file-chips">{filePaths.map((path) => <button key={path} onClick={() => setFilePaths((current) => current.filter((item) => item !== path))} title={t("Remove")}>{path.split(/[\\/]/).pop()} <span>×</span></button>)}</div>}
      {showLibrary && <div className="project-library-picker"><input value={search} onChange={(event) => setSearch(event.target.value)} placeholder={t("Search indexed files…")} /> <div>{visibleAssets.map((asset) => <label key={asset.id}><input type="checkbox" checked={assetIds.includes(asset.id)} onChange={(event) => { setAssetIds((current) => event.target.checked ? [...current, asset.id] : current.filter((id) => id !== asset.id)); if (event.target.checked && asset.storageMode === "managed") setMode("move"); }} /><span className="file-icon">{asset.extension.toUpperCase()}</span><span><strong>{asset.filename}</strong><small>{asset.projectName} · {asset.rootName} · {formatBytes(asset.byteSize)}</small></span>{assetIds.includes(asset.id) && <Check size={14} />}</label>)}</div></div>}
      <fieldset className="transfer-options"><legend>{t("What should Volum do with the selected files?")}</legend>
        <button type="button" className={mode === "move" ? "is-active is-recommended" : "is-recommended"} onClick={() => setMode("move")}><span><FolderInput size={18} /></span><strong>{t("Move into Volum")}</strong><small>{t("Recommended · one organized source of truth")}</small>{mode === "move" && <Check size={15} />}</button>
        <button type="button" className={mode === "copy" ? "is-active" : ""} onClick={() => setMode("copy")}><span><Copy size={18} /></span><strong>{t("Copy into Volum")}</strong><small>{t("Keep the originals where they are")}</small>{mode === "copy" && <Check size={15} />}</button>
        <button type="button" disabled={sourceCount === 0 || hasManagedSelection} className={mode === "link" ? "is-active" : ""} onClick={() => setMode("link")}><span><Link2 size={18} /></span><strong>{t("Keep linked")}</strong><small>{t("Leave selected files in their current folders")}</small>{mode === "link" && <Check size={15} />}</button>
      </fieldset>
      {mode !== "link" && <div className={`managed-destination ${managed?.root ? "is-ready" : ""}`}><span><FolderOpen size={18} /></span><div><strong>{managed?.root ? t("Managed library") : t("Managed library required")}</strong><small>{managed?.projectsPath ?? t("Choose where Volum should organize project folders.")}</small></div>{!managed?.root && <button className="button button--quiet" onClick={() => void configure()}>{t("Choose…")}</button>}</div>}
      {error && <p className="form-error">{error}</p>}
      <div className="dialog__actions"><button className="button button--quiet" onClick={onClose}>{t("Cancel")}</button><button className="button button--primary" disabled={!name.trim() || working || (mode !== "link" && !managed?.root)} onClick={() => void create()}>{working ? <LoaderCircle className="spin" size={15} /> : <Plus size={15} />}{t(working ? "Creating project…" : "Create project")}</button></div>
    </div>
  </Dialog>;
}
