import { useQuery, useQueryClient } from "@tanstack/react-query";
import {
  ArrowLeft,
  Box as BoxIcon,
  Check,
  CircleAlert,
  Crosshair,
  Eye,
  FileBox,
  Layers3,
  LoaderCircle,
  RefreshCw,
  RotateCcw,
  Search,
  ShieldCheck,
  UploadCloud,
  X,
} from "lucide-react";
import { lazy, Suspense, useCallback, useEffect, useMemo, useState } from "react";
import { formatBytes, formatNumber } from "../../lib/format";
import { localizedError, t, useLanguage } from "../../lib/i18n";
import { previewLocalStl, type LocalStlFile, type LocalStlPreview } from "../../lib/localStl";
import { notifyCompletion } from "../../lib/notifications";
import { cancelUpload, requestJson, uploadProjectFile } from "../../lib/transport";
import type { CompanionPage, Credential, ProjectSummary, ServerStatus, UploadProgress } from "../../types";

type Destination = "new" | "existing";
const MobileModelViewer = lazy(() => import("../../components/MobileModelViewer").then((module) => ({ default: module.MobileModelViewer })));

export function LocalStlPreviewScreen({
  file,
  credential,
  startSaving,
  onSavingStarted,
  onDiscard,
  onReplace,
  onPair,
  onSaved,
}: {
  file: LocalStlFile;
  credential?: Credential;
  startSaving?: boolean;
  onSavingStarted: () => void;
  onDiscard: () => void;
  onReplace: () => Promise<void>;
  onPair: () => void;
  onSaved: (projectId: string) => void;
}) {
  useLanguage();
  const queryClient = useQueryClient();
  const pairedCredential = credential;
  const [preview, setPreview] = useState<LocalStlPreview>();
  const [previewError, setPreviewError] = useState("");
  const [previewing, setPreviewing] = useState(true);
  const [resetSignal, setResetSignal] = useState(0);
  const [selectedObjectId, setSelectedObjectId] = useState<number>();
  const [isolatedObjectId, setIsolatedObjectId] = useState<number>();
  const [focusObjectId, setFocusObjectId] = useState<number>();
  const [focusSignal, setFocusSignal] = useState(0);
  const [objectsOpen, setObjectsOpen] = useState(false);
  const [replacing, setReplacing] = useState(false);
  const [replaceError, setReplaceError] = useState("");
  const [saveIntent, setSaveIntent] = useState(false);
  const [destination, setDestination] = useState<Destination>("new");
  const [projectName, setProjectName] = useState(() => file.filename.replace(/\.stl$/i, "").trim());
  const [projectId, setProjectId] = useState("");
  const [search, setSearch] = useState("");
  const [progress, setProgress] = useState<UploadProgress>();
  const [uploading, setUploading] = useState(false);
  const [uploadId, setUploadId] = useState("");
  const [canceling, setCanceling] = useState(false);
  const [saveError, setSaveError] = useState("");

  useEffect(() => {
    let active = true;
    setPreview(undefined);
    setPreviewError("");
    setPreviewing(true);
    setSelectedObjectId(undefined);
    setIsolatedObjectId(undefined);
    setFocusObjectId(undefined);
    setObjectsOpen(false);
    void previewLocalStl(file.cachePath)
      .then((next) => {
        if (active) setPreview(next);
      })
      .catch((reason) => {
        if (active) setPreviewError(localizedError(reason));
      })
      .finally(() => {
        if (active) setPreviewing(false);
      });
    return () => { active = false; };
  }, [file.cachePath]);

  useEffect(() => {
    if (!startSaving) return;
    onSavingStarted();
    setSaveIntent(true);
  }, [onSavingStarted, startSaving]);

  const status = useQuery({
    queryKey: ["local-stl-status", pairedCredential?.baseUrl],
    queryFn: () => requestJson<ServerStatus>(pairedCredential!, "/api/v1/status"),
    enabled: saveIntent && Boolean(pairedCredential),
    retry: false,
  });
  const projectPath = useMemo(() => {
    const parameters = new URLSearchParams({ limit: "100", sort: "modified" });
    if (search.trim()) parameters.set("search", search.trim());
    return `/api/v1/projects?${parameters}`;
  }, [search]);
  const projects = useQuery({
    queryKey: ["local-stl-projects", pairedCredential?.baseUrl, projectPath],
    queryFn: () => requestJson<CompanionPage<ProjectSummary>>(pairedCredential!, projectPath),
    enabled: saveIntent
      && destination === "existing"
      && Boolean(pairedCredential)
      && Boolean(status.data?.capabilities.fileUploads),
    retry: false,
  });

  const beginSaving = () => {
    setSaveError("");
    if (!pairedCredential) {
      onPair();
      return;
    }
    setSaveIntent(true);
  };
  const replace = async () => {
    setReplacing(true);
    setReplaceError("");
    try {
      await onReplace();
    } catch (reason) {
      setReplaceError(localizedError(reason));
    } finally {
      setReplacing(false);
    }
  };
  const save = async () => {
    if (!pairedCredential || uploading) return;
    const target = destination === "new"
      ? { projectName: projectName.trim() }
      : { projectId };
    if (!("projectName" in target ? target.projectName : target.projectId)) return;
    const nextUploadId = crypto.randomUUID();
    setUploadId(nextUploadId);
    setUploading(true);
    setSaveError("");
    setProgress(undefined);
    try {
      const result = await uploadProjectFile(
        pairedCredential,
        file.cachePath,
        file.filename,
        target,
        setProgress,
        nextUploadId,
      );
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["projects"] }),
        queryClient.invalidateQueries({ queryKey: ["status"] }),
      ]);
      await notifyCompletion(t("STL saved"), t("{filename} was added to Volum.", { filename: result.filename }));
      onSaved(result.projectId);
    } catch (reason) {
      const message = reason instanceof Error ? reason.message : String(reason);
      if (message !== "Upload canceled") setSaveError(localizedError(reason));
    } finally {
      setUploading(false);
      setUploadId("");
      setCanceling(false);
    }
  };
  const cancel = async () => {
    if (!uploadId || canceling) return;
    setCanceling(true);
    try {
      const canceled = await cancelUpload(uploadId);
      if (!canceled) {
        setSaveError(t("The upload is still starting or has already finished. Try again if it remains active."));
        setCanceling(false);
      }
    } catch (reason) {
      setSaveError(localizedError(reason));
      setCanceling(false);
    }
  };
  const closeSave = () => {
    if (uploading) return;
    setSaveIntent(false);
    setSaveError("");
  };
  const handleViewerStatus = useCallback(() => undefined, []);
  const percent = progress?.totalBytes
    ? Math.round(progress.uploadedBytes / progress.totalBytes * 100)
    : 0;
  const dimensions = preview?.dimensions
    .map((value) => `${formatNumber(value)} mm`)
    .join(" × ");
  const matchingProjects = projects.data?.items.filter((project) => !project.missing) ?? [];
  const selectedObject = preview?.objects.find((object) => object.id === selectedObjectId);
  const selectObject = (objectId?: number) => {
    if (!objectId) {
      setSelectedObjectId(undefined);
      setFocusObjectId(undefined);
      if (isolatedObjectId) {
        setIsolatedObjectId(undefined);
        setResetSignal((value) => value + 1);
      }
      return;
    }
    setSelectedObjectId(objectId);
    if (isolatedObjectId) {
      setIsolatedObjectId(objectId);
      setFocusObjectId(objectId);
      setFocusSignal((value) => value + 1);
    }
  };
  const focusObject = (objectId: number) => {
    setSelectedObjectId(objectId);
    setFocusObjectId(objectId);
    setFocusSignal((value) => value + 1);
  };
  const isolateObject = (objectId: number) => {
    setSelectedObjectId(objectId);
    setIsolatedObjectId(objectId);
    setFocusObjectId(objectId);
    setFocusSignal((value) => value + 1);
  };
  const showAllObjects = () => {
    setIsolatedObjectId(undefined);
    setFocusObjectId(undefined);
    setResetSignal((value) => value + 1);
  };

  return <main className="local-stl-screen">
    <header className="local-stl-header">
      <button className="round-button" aria-label={t("Discard STL")} disabled={uploading} onClick={onDiscard}><ArrowLeft /></button>
      <div><span>{t("STL PREVIEW")}</span><strong>{file.filename}</strong></div>
      <button className="round-button" aria-label={t("Replace STL")} disabled={uploading || replacing} onClick={() => void replace()}>{replacing ? <LoaderCircle className="spin" /> : <RefreshCw />}</button>
    </header>

    <section className="local-stl-stage" aria-label={t("On-device STL preview")}>
      {preview
        ? <Suspense fallback={<div className="local-stl-stage__state"><LoaderCircle className="spin" /><strong>{t("Opening 3D viewer…")}</strong></div>}><MobileModelViewer localPayload={preview.payload} resetSignal={resetSignal} selectedObjectId={selectedObjectId} isolatedObjectId={isolatedObjectId} focusObjectId={focusObjectId} focusSignal={focusSignal} onObjectSelect={selectObject} onStatus={handleViewerStatus} /></Suspense>
        : previewing
          ? <div className="local-stl-stage__state" role="status"><LoaderCircle className="spin" /><strong>{t("Building preview on this device…")}</strong><span>{t("The STL stays private while Volum prepares the viewer.")}</span></div>
          : <div className="local-stl-stage__state is-error" role="alert"><CircleAlert /><strong>{t("Preview unavailable")}</strong><span>{previewError || t("This STL could not be previewed on this device.")}</span><small>{t("The original file is still ready to save to Volum Desktop.")}</small></div>}
      {preview && <button className="local-stl-reset" aria-label={t("Reset 3D view")} onClick={() => setResetSignal((value) => value + 1)}><RotateCcw /></button>}
      {preview && preview.objects.length > 0 && <button className="local-stl-objects-button" aria-label={t("Open object list")} onClick={() => setObjectsOpen(true)}><Layers3 /><span>{preview.objects.length === 1 ? t("1 object") : t("{count} objects", { count: preview.objects.length })}</span></button>}
      {preview && !selectedObject && <div className="viewer-hint">{preview.objects.length ? t("Tap an object · Drag to rotate") : t("Drag to rotate · Pinch to zoom")}</div>}
    </section>

    <section className="local-stl-details">
      {selectedObject && <section className="local-object-inspector" aria-label={t("Selected object details")}>
        <header><div><span>{t("OBJECT {current} OF {count}", { current: selectedObject.id, count: preview?.objects.length ?? 0 })}</span><strong>{selectedObject.name || t("Object {number}", { number: selectedObject.id })}</strong></div><button className="round-button" aria-label={t("Clear object selection")} onClick={() => selectObject(undefined)}><X /></button></header>
        <div className="local-object-inspector__metrics">
          <div><span>{t("X · WIDTH")}</span><strong>{formatNumber(selectedObject.dimensions[0])} mm</strong></div>
          <div><span>{t("Y · DEPTH")}</span><strong>{formatNumber(selectedObject.dimensions[1])} mm</strong></div>
          <div><span>{t("Z · HEIGHT")}</span><strong>{formatNumber(selectedObject.dimensions[2])} mm</strong></div>
        </div>
        <p>{t("{triangles} triangles · Base Z {z} mm", { triangles: formatNumber(selectedObject.triangleCount), z: formatNumber(selectedObject.bounds[2]) })}</p>
        <div className="local-object-inspector__actions">
          <button className="button button--quiet" onClick={() => focusObject(selectedObject.id)}><Crosshair />{t("Fit to view")}</button>
          {isolatedObjectId
            ? <button className="button button--primary" onClick={showAllObjects}><Layers3 />{t("Show all")}</button>
            : <button className="button button--primary" onClick={() => isolateObject(selectedObject.id)}><Eye />{t("Inspect alone")}</button>}
        </div>
      </section>}
      <div className="local-stl-title"><span><FileBox /></span><div><h1>{file.filename.replace(/\.stl$/i, "")}</h1><p>STL · {formatBytes(file.byteSize)}</p></div></div>
      <div className="local-stl-metadata">
        <div><span>{t("Dimensions")}</span><strong>{dimensions || t("Unavailable")}</strong></div>
        <div><span>{t("Triangles")}</span><strong>{preview ? formatNumber(preview.triangleCount) : t("Unavailable")}</strong></div>
      </div>
      <div className="local-stl-privacy"><ShieldCheck /><span><strong>{t("Previewed on this device")}</strong>{t("Nothing is uploaded until you tap Save to Volum.")}</span></div>
      {replaceError && <div className="error-banner" role="alert"><CircleAlert />{replaceError}</div>}
      <div className="local-stl-actions">
        <button className="button button--quiet button--large" disabled={uploading || replacing} onClick={() => void replace()}>{t("Replace STL")}</button>
        <button className="button button--primary button--large" disabled={uploading} onClick={beginSaving}><UploadCloud />{pairedCredential ? t("Save to Volum") : t("Pair to save")}</button>
      </div>
      <button className="local-stl-discard" disabled={uploading} onClick={onDiscard}>{t("Discard local file")}</button>
    </section>

    {objectsOpen && preview && <div className="sheet-scrim" role="presentation">
      <section className="mobile-sheet mobile-sheet--tall local-objects-sheet" role="dialog" aria-modal="true" aria-labelledby="local-objects-title">
        <header><div><span className="eyebrow">{t("STL OBJECTS")}</span><h2 id="local-objects-title">{t("Inspect individual objects")}</h2></div><button className="round-button" aria-label={t("Close")} onClick={() => setObjectsOpen(false)}><X /></button></header>
        <p>{t("Objects are inferred from disconnected STL geometry.")}</p>
        <div className="local-objects-list">{preview.objects.map((object) => <button key={object.id} className={selectedObjectId === object.id ? "is-selected" : ""} aria-pressed={selectedObjectId === object.id} onClick={() => { selectObject(object.id); setObjectsOpen(false); }}><span><BoxIcon /></span><div><strong>{object.name || t("Object {number}", { number: object.id })}</strong><small>{object.dimensions.map((value) => `${formatNumber(value)} mm`).join(" × ")}</small><small>{t("{count} triangles", { count: formatNumber(object.triangleCount) })}</small></div>{selectedObjectId === object.id && <Check />}</button>)}</div>
      </section>
    </div>}

    {saveIntent && pairedCredential && <div className="sheet-scrim" role="presentation">
      <section className="mobile-sheet mobile-sheet--tall local-save-sheet" role="dialog" aria-modal="true" aria-labelledby="local-save-title">
        <header><div><span className="eyebrow">{t("SAVE TO VOLUM")}</span><h2 id="local-save-title">{t("Choose a destination")}</h2></div><button className="round-button" aria-label={t("Close")} disabled={uploading} onClick={closeSave}><X /></button></header>
        {status.isPending
          ? <div className="local-save-state" role="status"><LoaderCircle className="spin" /><strong>{t("Connecting to Volum Desktop…")}</strong><span>{t("Checking that your paired Desktop is ready.")}</span></div>
          : status.isError
            ? <div className="local-save-state is-error" role="alert"><CircleAlert /><strong>{t("Volum Desktop is unreachable")}</strong><span>{localizedError(status.error)}</span><button className="button button--quiet" onClick={() => void status.refetch()}><RefreshCw />{t("Try connection again")}</button></div>
            : !status.data?.capabilities.fileUploads
              ? <div className="local-save-state is-error" role="alert"><CircleAlert /><strong>{t("File saving is unavailable")}</strong><span>{t("Update Volum Desktop to enable secure file uploads.")}</span></div>
              : <>
                <div className="local-destination-tabs" role="tablist" aria-label={t("Destination type")}>
                  <button role="tab" aria-selected={destination === "new"} className={destination === "new" ? "is-active" : ""} disabled={uploading} onClick={() => setDestination("new")}><FileBox />{t("New project")}{destination === "new" && <Check />}</button>
                  <button role="tab" aria-selected={destination === "existing"} className={destination === "existing" ? "is-active" : ""} disabled={uploading} onClick={() => setDestination("existing")}><Search />{t("Existing project")}{destination === "existing" && <Check />}</button>
                </div>
                {destination === "new"
                  ? <label className="sheet-field"><span>{t("Project name")}</span><input value={projectName} maxLength={160} disabled={uploading} autoFocus onChange={(event) => setProjectName(event.target.value)} /></label>
                  : <div className="local-project-picker">
                    <label className="capture-project-search"><Search /><input value={search} disabled={uploading} placeholder={t("Search projects")} onChange={(event) => { setSearch(event.target.value); setProjectId(""); }} /></label>
                    {projects.isPending
                      ? <div className="local-project-picker__state"><LoaderCircle className="spin" />{t("Loading projects…")}</div>
                      : projects.isError
                        ? <div className="local-project-picker__state is-error"><CircleAlert />{localizedError(projects.error)}</div>
                        : matchingProjects.length
                          ? <div className="local-project-list">{matchingProjects.map((project) => <button key={project.id} className={projectId === project.id ? "is-selected" : ""} disabled={uploading} onClick={() => setProjectId(project.id)}><span>{project.primaryExtension || "3D"}</span><div><strong>{project.displayName}</strong><small>{project.folderName || t("Library")}</small></div>{projectId === project.id && <Check />}</button>)}</div>
                          : <div className="local-project-picker__state">{t("No matching projects")}</div>}
                  </div>}
                {uploading && <div className="upload-status" role="status" aria-live="polite"><div><LoaderCircle className="spin" /><strong>{canceling ? t("Canceling upload") : t("Saving to Desktop")}</strong><span>{percent}%</span></div><div className="upload-progress"><span style={{ width: `${percent}%` }} /></div><small>{t("Keep Volum open until the file is imported.")}</small><button className="cancel-upload-button" disabled={canceling} onClick={() => void cancel()}><X />{canceling ? t("Canceling…") : t("Cancel upload")}</button></div>}
                {saveError && <div className="error-banner" role="alert"><CircleAlert />{saveError}</div>}
                <button className="button button--primary button--large sheet-submit" disabled={uploading || (destination === "new" ? !projectName.trim() : !projectId)} onClick={() => void save()}>{uploading ? <LoaderCircle className="spin" /> : <UploadCloud />}{t("Save file")}</button>
              </>}
      </section>
    </div>}
  </main>;
}
