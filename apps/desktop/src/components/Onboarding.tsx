import { useState } from "react";
import { ArchiveRestore, FolderPlus, HardDrive, LoaderCircle, LockKeyhole, Sparkles } from "lucide-react";
import { open } from "@tauri-apps/plugin-dialog";
import { api, isTauri } from "../lib/tauri/api";
import { Brand } from "./Brand";
import { t } from "../lib/i18n";
import type { PreparedRestore } from "../types";
import { RestoreBackupDialog } from "../features/backups/BackupSettings";

export function Onboarding({ onComplete }: { onComplete: () => void }) {
  const [prepared, setPrepared] = useState<PreparedRestore>();
  const [busy, setBusy] = useState<"linked" | "managed" | "restore">();
  const [setupError, setSetupError] = useState("");
  const chooseManaged = async () => {
    if (busy) return;
    setBusy("managed");
    setSetupError("");
    try {
      const path = isTauri() ? await open({ directory: true, multiple: false, title: t("Choose the managed Volum library") }) : "/Users/you/Volum";
      if (!path || Array.isArray(path)) return;
      await api.configureManagedLibrary(path);
      onComplete();
    } catch (reason) {
      setSetupError(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setBusy(undefined);
    }
  };
  const chooseLinked = async () => {
    if (busy) return;
    setBusy("linked");
    setSetupError("");
    try {
      const path = isTauri() ? await open({ directory: true, multiple: false, title: t("Choose an existing 3D project folder") }) : "/Users/you/3D Projects";
      if (!path || Array.isArray(path)) return;
      const root = await api.addRoot(path);
      await api.startScan(root.id);
      onComplete();
    } catch (reason) {
      setSetupError(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setBusy(undefined);
    }
  };
  const chooseBackup = async () => {
    if (busy) return;
    setBusy("restore");
    setSetupError("");
    try {
      const path = isTauri() ? await open({ directory: false, multiple: false, title: t("Choose a Volum backup"), filters: [{ name: t("Volum backup"), extensions: ["zip"] }] }) : "/Users/you/Backups/volum-backup-demo.zip";
      if (!path || Array.isArray(path)) return;
      setPrepared(await api.prepareRestoreFromPath(path));
    } catch (reason) {
      setSetupError(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setBusy(undefined);
    }
  };
  return (
    <div className="onboarding">
      <div className="onboarding__panel"><Brand /><div className="onboarding__art"><div className="contour contour--1" /><div className="contour contour--2" /><div className="contour contour--3" /><div className="onboarding__object"><span /><span /><span /></div></div></div>
      <div className="onboarding__copy">
        <div className="eyebrow">{t("Welcome to Volum")}</div>
        <h1>{t("Bring your 3D projects into view.")}</h1>
        <p>{t("Start with a folder you already use. Volum turns it into a visual library while your files stay exactly where they are.")}</p>
        <div className="onboarding__promises">
          <span><LockKeyhole size={17} /><span><strong>{t("Private by design")}</strong><small>{t("No account or cloud required")}</small></span></span>
          <span><Sparkles size={17} /><span><strong>{t("Your files stay put")}</strong><small>{t("Indexed locally without moving or uploading them")}</small></span></span>
        </div>
        <div className="onboarding__choices">
          <article className="onboarding-choice onboarding-choice--primary">
            <span className="onboarding-choice__icon"><FolderPlus size={20} /></span>
            <div className="onboarding-choice__copy"><strong>{t("Use my existing projects")}</strong><p>{t("Choose a folder you already use. Volum indexes it in place and keeps watching for changes.")}</p></div>
            <button className="button button--primary" disabled={Boolean(busy)} onClick={() => void chooseLinked()}>{busy === "linked" ? <LoaderCircle className="spin" size={16} /> : <FolderPlus size={16} />}{t(busy === "linked" ? "Adding folder…" : "Choose a project folder")}</button>
          </article>
          <article className="onboarding-choice">
            <span className="onboarding-choice__icon"><HardDrive size={20} /></span>
            <div className="onboarding-choice__copy"><strong>{t("Start a new Volum library")}</strong><p>{t("Create an empty home for new projects Volum can organize, package, and include in backups.")}</p></div>
            <button className="button button--quiet" disabled={Boolean(busy)} onClick={() => void chooseManaged()}>{busy === "managed" ? <LoaderCircle className="spin" size={16} /> : <HardDrive size={16} />}{t(busy === "managed" ? "Setting up library…" : "Choose library location")}</button>
          </article>
        </div>
        {setupError && <div className="form-error onboarding__error" role="alert">{setupError}</div>}
        <div className="onboarding__restore"><span>{t("Returning to Volum?")}</span><button disabled={Boolean(busy)} onClick={() => void chooseBackup()}>{busy === "restore" ? <LoaderCircle className="spin" size={14} /> : <ArchiveRestore size={14} />}{t(busy === "restore" ? "Validating backup…" : "Restore from backup")}</button></div>
        <small className="onboarding__footnote">{t("Supports STL, 3MF, OBJ, STEP, and STP files")}</small>
      </div>
      {prepared && <RestoreBackupDialog prepared={prepared} onClose={() => setPrepared(undefined)} />}
    </div>
  );
}
