import { useEffect } from "react";
import { CheckCircle2, Download, LoaderCircle, RefreshCw, X } from "lucide-react";
import { useAppStore } from "../../app/store";
import { formatBytes } from "../../lib/format";
import { t } from "../../lib/i18n";
import { checkForUpdates, downloadUpdate, restartToUpdate, updatePercent, useUpdateStore } from "./updateStore";

const UPDATE_INTERVAL_MS = 6 * 60 * 60 * 1000;

export function UpdateManager() {
  const view = useAppStore((state) => state.view);
  useAppStore((state) => state.language);
  const { phase, update, downloadedBytes, totalBytes, downloadReady, error, dismissedVersion, dismissNotification } = useUpdateStore();

  useEffect(() => {
    void checkForUpdates(false);
    const interval = window.setInterval(() => void checkForUpdates(false), UPDATE_INTERVAL_MS);
    const checkAfterIdle = () => {
      const lastCheck = useUpdateStore.getState().checkedAt ?? 0;
      if (Date.now() - lastCheck >= UPDATE_INTERVAL_MS) void checkForUpdates(false);
    };
    window.addEventListener("focus", checkAfterIdle);
    return () => {
      window.clearInterval(interval);
      window.removeEventListener("focus", checkAfterIdle);
    };
  }, []);

  if (view === "settings" || !update) return null;
  if (["available", "downloaded", "error"].includes(phase) && dismissedVersion === update.version) return null;
  if (!["available", "downloading", "downloaded", "installing", "error"].includes(phase)) return null;

  const percent = updatePercent(downloadedBytes, totalBytes);
  const downloadError = phase === "error" && Boolean(update);
  return (
    <aside className="update-notification" aria-live="polite">
      <div className="update-notification__icon">
        {phase === "downloaded" ? <CheckCircle2 size={18} /> : phase === "downloading" || phase === "installing" ? <LoaderCircle className="spin" size={18} /> : <Download size={18} />}
      </div>
      <div className="update-notification__body">
        <strong>{phase === "downloaded" ? t("Update ready") : phase === "downloading" ? t("Downloading update…") : phase === "installing" ? t("Preparing update…") : downloadError ? t("Update failed") : t("Volum {version} is available", { version: update.version })}</strong>
        {phase === "available" && <small>{t("Download it when you’re ready.")}</small>}
        {phase === "downloading" && <><small>{totalBytes ? t("{downloaded} of {total}", { downloaded: formatBytes(downloadedBytes), total: formatBytes(totalBytes) }) : t("Downloading…")}</small><div className={`update-progress ${percent == null ? "is-indeterminate" : ""}`} role="progressbar" aria-label={t("Download progress")} aria-valuemin={0} aria-valuemax={100} aria-valuenow={percent}><i style={percent == null ? undefined : { width: `${percent}%` }} /></div></>}
        {phase === "downloaded" && <small>{t("Restart Volum to apply version {version}.", { version: update.version })}</small>}
        {phase === "installing" && <small>{t("Volum will reopen automatically.")}</small>}
        {downloadError && <small>{error || t("Couldn’t download the update.")}</small>}
        <div className="update-notification__actions">
          {phase === "available" && <><button className="button button--quiet" onClick={dismissNotification}>{t("Later")}</button><button className="button button--primary" onClick={() => void downloadUpdate()}><Download size={14} /> {t("Download update")}</button></>}
          {phase === "downloaded" && <><button className="button button--quiet" onClick={dismissNotification}>{t("Later")}</button><button className="button button--primary" onClick={() => void restartToUpdate()}><RefreshCw size={14} /> {t("Restart to update")}</button></>}
          {downloadError && <><button className="button button--quiet" onClick={dismissNotification}><X size={14} /> {t("Close")}</button><button className="button button--primary" onClick={() => void (downloadReady ? restartToUpdate() : downloadUpdate())}>{t("Try again")}</button></>}
        </div>
      </div>
    </aside>
  );
}
