import { useQueryClient } from "@tanstack/react-query";
import { open } from "@tauri-apps/plugin-dialog";
import { Camera, CircleAlert, LoaderCircle, ShieldCheck, UploadCloud, X } from "lucide-react";
import { useState } from "react";
import { Brand } from "../../components/Brand";
import { QuickCapturePanel } from "../capture/QuickCapturePanel";
import { formatBytes } from "../../lib/format";
import { localizedError, t } from "../../lib/i18n";
import { notifyCompletion } from "../../lib/notifications";
import { cancelUpload, filenameFromFilePath, uploadProjectFile } from "../../lib/transport";
import type { Credential, UploadProgress } from "../../types";

export function UploadScreen({ credential, enabled, photosEnabled, onOpen }: { credential: Credential; enabled: boolean; photosEnabled: boolean; onOpen: (id: string) => void }) {
  const queryClient = useQueryClient();
  const [mode, setMode] = useState<"file" | "photo">("photo");
  const [filePath, setFilePath] = useState("");
  const [filename, setFilename] = useState("");
  const [projectName, setProjectName] = useState("");
  const [progress, setProgress] = useState<UploadProgress>();
  const [uploading, setUploading] = useState(false);
  const [uploadId, setUploadId] = useState("");
  const [canceling, setCanceling] = useState(false);
  const [error, setError] = useState("");
  const choose = async () => {
    setError("");
    try {
      const selected = await open({ multiple: false, directory: false, title: t("Choose a 3D file"), filters: [{ name: t("3D files"), extensions: ["stl", "3mf", "obj", "step", "stp"] }] });
      if (!selected || Array.isArray(selected)) return;
      const nextFilename = filenameFromFilePath(selected);
      setFilePath(selected); setFilename(nextFilename); setProjectName(nextFilename.replace(/\.[^.]+$/, "")); setProgress(undefined);
    } catch (reason) { setError(localizedError(reason)); }
  };
  const upload = async () => {
    if (!filePath || !filename || !projectName.trim()) return;
    setUploading(true); setError(""); setProgress(undefined);
    const nextUploadId = crypto.randomUUID();
    setUploadId(nextUploadId);
    try {
      const result = await uploadProjectFile(credential, filePath, filename, { projectName: projectName.trim() }, setProgress, nextUploadId);
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["projects"] }),
        queryClient.invalidateQueries({ queryKey: ["status"] })
      ]);
      await notifyCompletion(t("3D file imported"), t("{filename} is now in Volum.", { filename: result.filename }));
      onOpen(result.projectId);
    } catch (reason) {
      const message = reason instanceof Error ? reason.message : String(reason);
      if (message !== "Upload canceled") setError(localizedError(reason));
    } finally { setUploading(false); setUploadId(""); setCanceling(false); }
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
  const percent = progress?.totalBytes ? Math.round(progress.uploadedBytes / progress.totalBytes * 100) : 0;
  return <main className="library-screen upload-screen"><header className="mobile-header"><div><div className="eyebrow">{t("WORKSHOP CAPTURE")}</div><h1>{t("Capture")}</h1><p>{t("Add a project photo or send a new print file")}</p></div><Brand compact /></header><div className="capture-mode" role="tablist" aria-label={t("Capture type")}><button role="tab" aria-selected={mode === "photo"} className={mode === "photo" ? "is-active" : ""} onClick={() => setMode("photo")}><Camera />{t("Photo")}</button><button role="tab" aria-selected={mode === "file"} className={mode === "file" ? "is-active" : ""} onClick={() => setMode("file")}><UploadCloud />{t("3D file")}</button></div>{mode === "photo" ? <QuickCapturePanel credential={credential} enabled={photosEnabled} onOpen={onOpen} /> : <><section className="upload-card"><span className="upload-card__icon"><UploadCloud /></span><h2>{filename || t("Choose a 3D file")}</h2><p>{filename ? `${progress ? `${formatBytes(progress.totalBytes)} · ` : ""}${t("Ready for Volum Desktop")}` : t("STL, 3MF, OBJ, STEP, and STP up to 500 MB.")}</p><button className="button button--quiet button--large" disabled={uploading} onClick={() => void choose()}>{filename ? t("Choose another file") : t("Browse Files")}</button>{filename && <label className="upload-name"><span>{t("New project name")}</span><input value={projectName} maxLength={160} disabled={uploading} onChange={(event) => setProjectName(event.target.value)} /></label>}{uploading && <div className="upload-status" role="status" aria-live="polite"><div><LoaderCircle className="spin" /><strong>{canceling ? t("Canceling upload") : t("Uploading to Desktop")}</strong><span>{percent}%</span></div><div className="upload-progress"><span style={{ width: `${percent}%` }} /></div><small>{t("Keep Volum open until the file is imported.")}</small><button className="cancel-upload-button" disabled={canceling} onClick={() => void cancel()}><X />{canceling ? t("Canceling…") : t("Cancel upload")}</button></div>}{error && <div className="error-banner" role="alert"><CircleAlert size={16} />{error}</div>}<button className="button button--primary button--large" disabled={!enabled || !filePath || !projectName.trim() || uploading} onClick={() => void upload()}>{uploading ? <LoaderCircle className="spin" /> : <UploadCloud />}{uploading ? `${t("Uploading")} ${percent}%` : t("Upload to Volum")}</button>{!enabled && <small className="upload-unavailable">{t("Update Volum Desktop to enable secure file uploads.")}</small>}</section><p className="upload-privacy"><ShieldCheck />{t("Sent directly through your Tailnet. No Volum cloud or relay.")}</p></>}</main>;
}
