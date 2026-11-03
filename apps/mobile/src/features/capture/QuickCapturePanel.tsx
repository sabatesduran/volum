import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Camera, Check, CircleAlert, FileImage, LoaderCircle, Search, ShieldCheck, X } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { cancelUpload, requestJson, uploadProjectPhoto } from "../../lib/transport";
import { notifyCompletion } from "../../lib/notifications";
import { localizedError, t, useLanguage } from "../../lib/i18n";
import type {
  CompanionPage,
  Credential,
  ProjectDocumentation,
  ProjectSummary,
  UploadProgress,
} from "../../types";

export function QuickCapturePanel({
  credential,
  enabled,
  onOpen,
}: {
  credential: Credential;
  enabled: boolean;
  onOpen: (id: string) => void;
}) {
  useLanguage();
  const queryClient = useQueryClient();
  const input = useRef<HTMLInputElement>(null);
  const [projectId, setProjectId] = useState("");
  const [search, setSearch] = useState("");
  const [file, setFile] = useState<File>();
  const [preview, setPreview] = useState("");
  const [caption, setCaption] = useState("");
  const [progress, setProgress] = useState<UploadProgress>();
  const [uploading, setUploading] = useState(false);
  const [uploadId, setUploadId] = useState("");
  const [canceling, setCanceling] = useState(false);
  const [error, setError] = useState("");
  const projects = useQuery({
    queryKey: ["quick-capture-projects"],
    queryFn: () => requestJson<CompanionPage<ProjectSummary>>(
      credential,
      "/api/v1/projects?limit=100&sort=modified",
    ),
    enabled,
  });
  const visibleProjects = useMemo(() => {
    const needle = search.trim().toLocaleLowerCase();
    const items = projects.data?.items ?? [];
    return needle
      ? items.filter((project) => `${project.displayName} ${project.folderName}`.toLocaleLowerCase().includes(needle))
      : items;
  }, [projects.data?.items, search]);

  useEffect(() => {
    if (!file) { setPreview(""); return; }
    const url = URL.createObjectURL(file);
    setPreview(url);
    return () => URL.revokeObjectURL(url);
  }, [file]);

  const choose = (capture: boolean) => {
    if (!input.current) return;
    if (capture) input.current.setAttribute("capture", "environment");
    else input.current.removeAttribute("capture");
    input.current.click();
  };

  const upload = async () => {
    if (!file || !projectId) return;
    setUploading(true); setError(""); setProgress(undefined);
    const nextUploadId = crypto.randomUUID();
    setUploadId(nextUploadId);
    try {
      const documentation = await requestJson<ProjectDocumentation>(
        credential,
        `/api/v1/projects/${encodeURIComponent(projectId)}/documentation`,
      );
      await uploadProjectPhoto(
        credential,
        file,
        projectId,
        caption,
        documentation.revision,
        setProgress,
        nextUploadId,
      );
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["project", projectId] }),
        queryClient.invalidateQueries({ queryKey: ["documentation", projectId] }),
        queryClient.invalidateQueries({ queryKey: ["projects"] }),
        queryClient.invalidateQueries({ queryKey: ["status"] }),
      ]);
      const project = projects.data?.items.find((item) => item.id === projectId);
      await notifyCompletion(t("Photo added"), project ? t("Saved to {name}.", { name: project.displayName }) : t("Saved to your Volum project."));
      onOpen(projectId);
    } catch (reason) {
      const message = reason instanceof Error ? reason.message : String(reason);
      if (message !== "Upload canceled") setError(localizedError(reason));
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
        setError(t("The upload is still starting or has already finished. Try again if it remains active."));
        setCanceling(false);
      }
    }
    catch (reason) { setError(localizedError(reason)); setCanceling(false); }
  };

  const percent = progress?.totalBytes
    ? Math.round(progress.uploadedBytes / progress.totalBytes * 100)
    : 0;

  return <section className="quick-capture-card">
    <input
      ref={input}
      className="visually-hidden"
      type="file"
      accept="image/jpeg,image/png,image/webp"
      onChange={(event) => {
        setFile(event.target.files?.[0]);
        setProgress(undefined);
        setError("");
        event.target.value = "";
      }}
    />
    <div className="quick-capture-card__hero">
      {preview ? <img src={preview} alt={t("Selected workshop photo")} /> : <Camera />}
    </div>
    <div className="quick-capture-card__actions">
      <button disabled={!enabled || uploading} onClick={() => choose(true)}><Camera />{t("Take photo")}</button>
      <button disabled={!enabled || uploading} onClick={() => choose(false)}><FileImage />{t("Photo library")}</button>
    </div>
    <label className="upload-name">
      <span>{t("Project")}</span>
      <div className="capture-project-search"><Search /><input value={search} placeholder={t("Filter projects")} onChange={(event) => setSearch(event.target.value)} /></div>
      <select value={projectId} disabled={uploading || projects.isLoading} onChange={(event) => setProjectId(event.target.value)}>
        <option value="">{t("Choose a project")}</option>
        {visibleProjects.map((project) => <option key={project.id} value={project.id}>{project.displayName} · {project.folderName}</option>)}
      </select>
    </label>
    {file && <label className="upload-name"><span>{t("Caption")}</span><input value={caption} maxLength={500} disabled={uploading} placeholder={t("What does this photo show?")} onChange={(event) => setCaption(event.target.value)} /></label>}
    {uploading && <div className="upload-status" role="status" aria-live="polite"><div><LoaderCircle className="spin" /><strong>{canceling ? t("Canceling upload") : t("Sending photo")}</strong><span>{percent}%</span></div><div className="upload-progress"><span style={{ width: `${percent}%` }} /></div><small>{t("Volum Desktop validates and stores the original.")}</small><button className="cancel-upload-button" disabled={canceling} onClick={() => void cancel()}><X />{canceling ? t("Canceling…") : t("Cancel upload")}</button></div>}
    {error && <div className="error-banner" role="alert"><CircleAlert />{error}</div>}
    <button className="button button--primary button--large" disabled={!enabled || !projectId || !file || uploading} onClick={() => void upload()}>{uploading ? <LoaderCircle className="spin" /> : <Check />}{uploading ? `${t("Uploading")} ${percent}%` : t("Add photo to project")}</button>
    {!enabled && <small className="upload-unavailable">{t("Enable organization writes in Volum Desktop to add photos.")}</small>}
    <p className="upload-privacy"><ShieldCheck />{t("Sent directly through your chosen network. No Volum cloud or relay.")}</p>
  </section>;
}
