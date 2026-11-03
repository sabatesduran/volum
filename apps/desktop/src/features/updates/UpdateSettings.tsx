import { CheckCircle2, Download, LoaderCircle, RefreshCw, TriangleAlert } from "lucide-react";
import { formatBytes, formatDate } from "../../lib/format";
import { t } from "../../lib/i18n";
import { checkForUpdates, downloadUpdate, restartToUpdate, updatePercent, useUpdateStore } from "./updateStore";

export function UpdateSettings() {
  const { phase, update, downloadedBytes, totalBytes, downloadReady, error } = useUpdateStore();
  const percent = updatePercent(downloadedBytes, totalBytes);
  const checkDisabled = ["checking", "available", "downloading", "downloaded", "installing"].includes(phase) || (phase === "error" && Boolean(update));

  return (
    <section className="settings-section" id="updates">
      <div className="settings-section__header">
        <div><h2>{t("Updates")}</h2><p>{t("Updates are checked at startup and every six hours. Nothing is downloaded without your approval.")}</p></div>
        <button className="button button--quiet" disabled={checkDisabled} onClick={() => void checkForUpdates(true)}>
          {phase === "checking" ? <LoaderCircle className="spin" size={15} /> : <Download size={15} />} {t(phase === "checking" ? "Checking…" : "Check for updates")}
        </button>
      </div>

      {phase === "idle" && <p className="settings-footnote">{t("Signed update packages are verified before installation.")}</p>}
      {phase === "up-to-date" && <div className="update-status"><CheckCircle2 size={16} />{t("Volum is up to date.")}</div>}
      {phase === "preview" && <div className="update-status"><CheckCircle2 size={16} />{t("You’re using the browser preview.")}</div>}
      {phase === "checking" && <div className="update-status"><LoaderCircle className="spin" size={16} />{t("Checking for updates…")}</div>}

      {update && ["available", "downloading", "downloaded", "installing"].includes(phase) && (
        <div className="update-card">
          <div className="update-card__header"><div><strong>{t("Volum {version}", { version: update.version })}</strong>{update.date && <small>{t("Released {date}", { date: formatDate(update.date) })}</small>}</div><span>{phase === "available" ? t("Available") : phase === "downloaded" ? t("Ready") : phase === "installing" ? t("Installing") : t("Downloading")}</span></div>
          {update.body && <div className="update-release-notes"><strong>{t("Release notes")}</strong><p>{update.body}</p></div>}
          {phase === "downloading" && <div className="update-download"><div><span>{t("Downloading update…")}</span><small>{totalBytes ? t("{downloaded} of {total}", { downloaded: formatBytes(downloadedBytes), total: formatBytes(totalBytes) }) : t("Downloading…")}</small></div><div className={`update-progress ${percent == null ? "is-indeterminate" : ""}`} role="progressbar" aria-label={t("Download progress")} aria-valuemin={0} aria-valuemax={100} aria-valuenow={percent}><i style={percent == null ? undefined : { width: `${percent}%` }} /></div></div>}
          {phase === "downloaded" && <p className="update-card__message">{t("The update is downloaded and ready. Restart Volum when it’s convenient.")}</p>}
          {phase === "installing" && <div className="update-status"><LoaderCircle className="spin" size={16} />{t("Preparing update…")}</div>}
          <div className="update-card__actions">
            {phase === "available" && <button className="button button--primary" onClick={() => void downloadUpdate()}><Download size={15} /> {t("Download update")}</button>}
            {phase === "downloaded" && <button className="button button--primary" onClick={() => void restartToUpdate()}><RefreshCw size={15} /> {t("Restart to update")}</button>}
          </div>
        </div>
      )}

      {phase === "error" && <div className="update-error"><TriangleAlert size={16} /><span><strong>{t("Update failed")}</strong><small>{error || t("Couldn’t complete the update.")}</small></span><button className="button button--quiet" onClick={() => void (downloadReady ? restartToUpdate() : update ? downloadUpdate() : checkForUpdates(true))}>{t("Try again")}</button></div>}
    </section>
  );
}
