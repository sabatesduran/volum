import { useState } from "react";
import { ArchiveRestore, FolderPlus, LoaderCircle, LockKeyhole, Sparkles } from "lucide-react";
import { open } from "@tauri-apps/plugin-dialog";
import { api, isTauri } from "../lib/tauri/api";
import { Brand } from "./Brand";
import { t } from "../lib/i18n";
import type { PreparedRestore } from "../types";
import { RestoreBackupDialog } from "../features/backups/BackupSettings";

export function Onboarding({ onComplete }: { onComplete: () => void }) {
  const [prepared, setPrepared] = useState<PreparedRestore>();
  const [restoring, setRestoring] = useState(false);
  const [restoreError, setRestoreError] = useState("");
  const choose = async () => {
    const path = isTauri() ? await open({ directory: true, multiple: false, title: t("Choose your 3D model library") }) : "/Users/you/3D Models";
    if (!path || Array.isArray(path)) return;
    const root = await api.addRoot(path);
    onComplete();
    await api.startScan(root.id);
  };
  const chooseBackup = async () => {
    const path = isTauri() ? await open({ directory: false, multiple: false, title: t("Choose a Volum backup"), filters: [{ name: t("Volum backup"), extensions: ["zip"] }] }) : "/Users/you/Backups/volum-backup-demo.zip";
    if (!path || Array.isArray(path)) return;
    setRestoring(true);
    setRestoreError("");
    try {
      setPrepared(await api.prepareRestoreFromPath(path));
    } catch (reason) {
      setRestoreError(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setRestoring(false);
    }
  };
  return (
    <div className="onboarding">
      <div className="onboarding__panel"><Brand /><div className="onboarding__art"><div className="contour contour--1" /><div className="contour contour--2" /><div className="contour contour--3" /><div className="onboarding__object"><span /><span /><span /></div></div></div>
      <div className="onboarding__copy"><div className="eyebrow">{t("Welcome to Volum")}</div><h1>{t("A beautiful home")}<br />{t("for your 3D models.")}</h1><p>{t("Choose one or more folders. Volum indexes them locally without moving or uploading a single file.")}</p><div className="onboarding__promises"><span><LockKeyhole size={17} /><span><strong>{t("Private by design")}</strong><small>{t("No account or cloud required")}</small></span></span><span><Sparkles size={17} /><span><strong>{t("Your folders, enriched")}</strong><small>{t("Automatic previews and metadata")}</small></span></span></div><div className="onboarding__actions"><button className="button button--primary button--large" onClick={choose}><FolderPlus size={18} /> {t("Choose a model folder")}</button><button className="button button--quiet button--large" disabled={restoring} onClick={() => void chooseBackup()}>{restoring ? <LoaderCircle className="spin" size={18} /> : <ArchiveRestore size={18} />} {t(restoring ? "Validating backup…" : "Restore a backup…")}</button></div>{restoreError && <div className="form-error">{restoreError}</div>}<small className="onboarding__footnote">{t("Supports STL, 3MF, OBJ, STEP, and STP files")}</small></div>
      {prepared && <RestoreBackupDialog prepared={prepared} onClose={() => setPrepared(undefined)} />}
    </div>
  );
}
