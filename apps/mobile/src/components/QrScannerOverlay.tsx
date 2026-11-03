import { QrCode, X } from "lucide-react";
import { useEffect } from "react";
import { t } from "../lib/i18n";

export function QrScannerOverlay({
  title,
  instructions,
  onCancel,
}: {
  title: string;
  instructions: string;
  onCancel: () => void;
}) {
  useEffect(() => {
    document.documentElement.classList.add("is-barcode-scanning");
    document.body.classList.add("is-barcode-scanning");
    return () => {
      document.documentElement.classList.remove("is-barcode-scanning");
      document.body.classList.remove("is-barcode-scanning");
    };
  }, []);

  return <div className="barcode-scanner-overlay" role="dialog" aria-modal="true" aria-labelledby="barcode-scanner-title">
    <header>
      <span><QrCode />{t("QR scanner")}</span>
      <button className="round-button" aria-label={t("Cancel scan")} onClick={onCancel}><X /></button>
    </header>
    <div className="barcode-scanner-frame" aria-hidden="true"><i /><i /><i /><i /></div>
    <footer>
      <h2 id="barcode-scanner-title">{title}</h2>
      <p>{instructions}</p>
      <button className="button button--large" onClick={onCancel}><X />{t("Cancel scan")}</button>
    </footer>
  </div>;
}
