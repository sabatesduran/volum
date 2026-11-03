import { useEffect, useState } from "react";
import { LoaderCircle, QrCode, Smartphone } from "lucide-react";
import QRCode from "qrcode";
import { Dialog } from "../../components/Dialog";
import { t } from "../../lib/i18n";
import { projectQrPayload } from "../../lib/projectQr";

export type ProjectQrReadiness = "ready" | "access-off" | "unpaired" | "unavailable";

export function ProjectQrDialog({
  serverId,
  projectId,
  projectName,
  readiness,
  onClose,
  onOpenCompanion,
}: {
  serverId?: string;
  projectId: string;
  projectName: string;
  readiness: ProjectQrReadiness;
  onClose: () => void;
  onOpenCompanion?: () => void;
}) {
  const [qrUrl, setQrUrl] = useState("");
  const [error, setError] = useState("");

  useEffect(() => {
    if (readiness !== "ready" || !serverId) return;
    let cancelled = false;
    setQrUrl("");
    setError("");
    void QRCode.toDataURL(projectQrPayload(serverId, projectId), {
      width: 360,
      margin: 3,
      color: { dark: "#191916", light: "#fffefb" },
      errorCorrectionLevel: "M",
    }).then((url) => {
      if (!cancelled) setQrUrl(url);
    }).catch(() => {
      if (!cancelled) setError(t("Couldn’t create the project QR code."));
    });
    return () => { cancelled = true; };
  }, [projectId, readiness, serverId]);

  const unavailableCopy = readiness === "unpaired"
    ? t("Pair a phone before opening this project on mobile.")
    : readiness === "access-off"
      ? t("Companion Access must be running before your phone can open this project.")
      : t("This Desktop is not ready to share projects with mobile.");

  return <Dialog
    title={t("Open this project on your phone")}
    subtitle={readiness === "ready"
      ? t("On your paired phone, open Library, tap the QR scanner, and scan this code.")
      : unavailableCopy}
    onClose={onClose}
    size="medium"
  >
    {readiness === "ready" ? <div className="project-handoff">
      <div className="project-handoff__code">
        {qrUrl ? <img src={qrUrl} alt={t("Project QR code for {name}", { name: projectName })} /> : error ? <span className="form-error">{error}</span> : <LoaderCircle className="spin" size={28} />}
      </div>
      <strong>{projectName}</strong>
      <p>{t("This code contains only the server and project IDs—never your token or file paths.")}</p>
    </div> : <div className="project-handoff project-handoff--setup">
      <span className="project-handoff__icon">{readiness === "unpaired" ? <Smartphone size={28} /> : <QrCode size={28} />}</span>
      <strong>{t("Companion Access")}</strong>
      {onOpenCompanion && <button className="button button--primary" onClick={onOpenCompanion}>{t("Open Companion Access")}</button>}
    </div>}
  </Dialog>;
}
