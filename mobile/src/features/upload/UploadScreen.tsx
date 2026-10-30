import { open } from "@tauri-apps/plugin-dialog";
import { Camera, Check, CircleAlert, FileBox, LoaderCircle, ShieldCheck, UploadCloud } from "lucide-react";
import { useState } from "react";
import { QuickCapturePanel } from "../capture/QuickCapturePanel";
import { localizedError, t } from "../../lib/i18n";
import { filenameFromFilePath } from "../../lib/transport";
import type { Credential } from "../../types";

export function UploadScreen({
  credential,
  photosEnabled,
  onOpen,
  onPreviewStl,
}: {
  credential: Credential;
  photosEnabled: boolean;
  onOpen: (id: string) => void;
  onPreviewStl: (filePath: string, filename: string) => Promise<void>;
}) {
  const [mode, setMode] = useState<"file" | "photo">("photo");
  const [choosing, setChoosing] = useState(false);
  const [error, setError] = useState("");
  const choose = async () => {
    setChoosing(true);
    setError("");
    try {
      const selected = await open({
        multiple: false,
        directory: false,
        title: t("Choose an STL file to preview"),
        filters: [{ name: t("STL files"), extensions: ["stl"] }],
      });
      if (!selected || Array.isArray(selected)) return;
      await onPreviewStl(selected, filenameFromFilePath(selected));
    } catch (reason) {
      setError(localizedError(reason));
    } finally {
      setChoosing(false);
    }
  };
  return <main className="library-screen upload-screen">
    <header className="mobile-header"><div><h1>{t("Capture")}</h1><p>{t("Add a project photo or open an STL on this device")}</p></div></header>
    <div className="capture-mode" role="tablist" aria-label={t("Capture type")}>
      <button role="tab" aria-selected={mode === "photo"} className={mode === "photo" ? "is-active" : ""} onClick={() => setMode("photo")}><Camera /><span>{t("Photo")}</span>{mode === "photo" && <Check className="capture-mode__check" />}</button>
      <button role="tab" aria-selected={mode === "file"} className={mode === "file" ? "is-active" : ""} onClick={() => setMode("file")}><UploadCloud /><span>{t("STL file")}</span>{mode === "file" && <Check className="capture-mode__check" />}</button>
    </div>
    {mode === "photo"
      ? <QuickCapturePanel credential={credential} enabled={photosEnabled} onOpen={onOpen} />
      : <>
        <section className="upload-card local-picker-card">
          <span className="upload-card__icon"><FileBox /></span>
          <h2>{t("Open an STL")}</h2>
          <p>{t("Inspect the model privately on this device, then choose whether and where to save it.")}</p>
          <button className="button button--primary button--large" disabled={choosing} onClick={() => void choose()}>{choosing ? <LoaderCircle className="spin" /> : <UploadCloud />}{t("Browse STL files")}</button>
          <div className="local-picker-promise"><ShieldCheck /><span><strong>{t("Local first")}</strong>{t("No network request is made until you tap Save to Volum.")}</span></div>
          <small>{t("STL files up to 500 MB. Large or detailed models may skip the preview but can still be saved.")}</small>
        </section>
        {error && <div className="error-banner" role="alert"><CircleAlert />{error}</div>}
      </>}
  </main>;
}
