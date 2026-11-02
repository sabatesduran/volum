import { CircleAlert, LoaderCircle, RefreshCw, WifiOff } from "lucide-react";
import { localizedError, t } from "../lib/i18n";
import { isConnectionError } from "../lib/transport";

export function ErrorState({ error, onRetry, retrying = false }: { error: unknown; onRetry: () => void; retrying?: boolean }) {
  const connectionError = isConnectionError(error);
  return <div className={`center-state error-state ${connectionError ? "is-connection" : ""}`} role="alert">
    <span className="error-state__icon">{connectionError ? <WifiOff /> : <CircleAlert />}</span>
    <strong>{t(connectionError ? "Could not connect to Volum Desktop" : "Could not load Volum")}</strong>
    <p>{connectionError
      ? t("Check that the selected private network or HTTPS address is reachable, Volum Desktop is open, and Companion Access is on.")
      : localizedError(error)}</p>
    <button className="button button--primary error-state__retry" disabled={retrying} onClick={onRetry}>
      {retrying ? <LoaderCircle className="spin" /> : <RefreshCw />}
      {t(retrying ? "Connecting…" : "Try again")}
    </button>
  </div>;
}
