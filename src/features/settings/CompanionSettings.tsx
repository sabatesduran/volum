import { useEffect, useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Check, Copy, Laptop, LoaderCircle, PencilLine, QrCode, ShieldCheck, Smartphone, Trash2, Wifi, WifiOff } from "lucide-react";
import QRCode from "qrcode";
import { Dialog } from "../../components/Dialog";
import { formatDate } from "../../lib/format";
import { t } from "../../lib/i18n";
import { api } from "../../lib/tauri/api";
import type { CompanionPairingInfo } from "../../types";

function PairingDialog({ pairing, onClose }: { pairing: CompanionPairingInfo; onClose: () => void }) {
  const [qrUrl, setQrUrl] = useState("");
  const [copied, setCopied] = useState(false);
  const [remaining, setRemaining] = useState(0);

  useEffect(() => {
    let cancelled = false;
    void QRCode.toDataURL(pairing.qrPayload, {
      width: 280,
      margin: 2,
      color: { dark: "#191916", light: "#fffefb" },
      errorCorrectionLevel: "M"
    }).then((url) => { if (!cancelled) setQrUrl(url); });
    return () => { cancelled = true; };
  }, [pairing.qrPayload]);

  useEffect(() => {
    const update = () => setRemaining(Math.max(0, Math.ceil((Date.parse(pairing.expiresAt) - Date.now()) / 1000)));
    update();
    const timer = window.setInterval(update, 1_000);
    return () => window.clearInterval(timer);
  }, [pairing.expiresAt]);

  const time = `${Math.floor(remaining / 60)}:${String(remaining % 60).padStart(2, "0")}`;
  const copyAddress = async () => {
    await navigator.clipboard.writeText(pairing.baseUrl);
    setCopied(true);
    window.setTimeout(() => setCopied(false), 1_500);
  };

  return (
    <Dialog
      title={t("Pair Volum on mobile")}
      subtitle={t("Open Volum on your phone and scan this code. Both devices must be signed in to your Tailscale network.")}
      onClose={onClose}
      size="medium"
    >
      <div className="companion-pairing">
        <div className={`companion-pairing__code ${remaining === 0 ? "is-expired" : ""}`}>
          {qrUrl ? <img src={qrUrl} alt={t("Companion pairing QR code")} /> : <LoaderCircle className="spin" size={28} />}
          {remaining === 0 && <span>{t("Pairing code expired")}</span>}
        </div>
        <div className="companion-pairing__details">
          <div><span>{t("Desktop")}</span><strong>{pairing.serverName}</strong></div>
          <div><span>{t("Tailnet address")}</span><strong>{pairing.baseUrl}</strong></div>
          <div><span>{t("Expires in")}</span><strong>{time}</strong></div>
        </div>
        <button className="button button--quiet" onClick={() => void copyAddress()}>
          {copied ? <Check size={15} /> : <Copy size={15} />}
          {t(copied ? "Copied" : "Copy address")}
        </button>
        <p className="settings-footnote">{t("The QR code contains a one-time secret. It stops working after one successful pairing or five minutes.")}</p>
      </div>
    </Dialog>
  );
}

export function CompanionSettings() {
  const queryClient = useQueryClient();
  const [pairing, setPairing] = useState<CompanionPairingInfo>();
  const [error, setError] = useState("");
  const { data: status, isLoading } = useQuery({
    queryKey: ["companion-status"],
    queryFn: api.companionStatus,
    refetchInterval: 15_000
  });
  const toggle = useMutation({
    mutationFn: (enabled: boolean) => api.setCompanionEnabled(enabled),
    onSuccess: (next) => { queryClient.setQueryData(["companion-status"], next); setError(next.lastError ?? ""); },
    onError: (reason) => setError(reason instanceof Error ? reason.message : String(reason))
  });
  const writeToggle = useMutation({
    mutationFn: (enabled: boolean) => api.setCompanionOrganizationWrites(enabled),
    onSuccess: (next) => { queryClient.setQueryData(["companion-status"], next); setError(""); },
    onError: (reason) => setError(reason instanceof Error ? reason.message : String(reason))
  });
  const runningLabel = useMemo(() => {
    if (isLoading) return t("Checking…");
    if (status?.running) return t("Available on your Tailnet");
    if (status?.enabled) return t("Needs attention");
    return t("Off");
  }, [isLoading, status?.enabled, status?.running]);

  const pairDevice = async () => {
    setError("");
    try { setPairing(await api.createCompanionPairing()); }
    catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)); }
  };
  const revoke = async (deviceId: string, name: string) => {
    if (!window.confirm(t("Revoke access for {name}?", { name }))) return;
    const next = await api.revokeCompanionDevice(deviceId);
    queryClient.setQueryData(["companion-status"], next);
  };

  return (
    <section className="settings-section" id="companion">
      <div className="settings-section__header">
        <div><h2>{t("Companion Access")}</h2><p>{t("Browse your Volum library from iPhone, iPad, or Android anywhere your private Tailscale network reaches.")}</p></div>
        <button
          className={`settings-switch ${status?.enabled ? "is-on" : ""}`}
          role="switch"
          aria-checked={Boolean(status?.enabled)}
          disabled={isLoading || toggle.isPending}
          onClick={() => toggle.mutate(!status?.enabled)}
        ><span /></button>
      </div>
      <div className={`companion-status ${status?.running ? "is-running" : ""}`}>
        <span className="companion-status__icon">{status?.running ? <Wifi size={20} /> : <WifiOff size={20} />}</span>
        <div><strong>{runningLabel}</strong><small>{status?.running ? t("Encrypted by Tailscale and protected by a Volum device token.") : t("Turn on access while Volum Desktop is running to connect your phone.")}</small></div>
        {status?.tailnetIp && <code>{status.tailnetIp}:{status.port}</code>}
      </div>
      {status?.enabled && <div className="companion-permission">
        <span className="companion-permission__icon"><PencilLine size={17} /></span>
        <div><strong>{t("Allow organization changes")}</strong><small>{t("Let paired mobile devices edit project names, notes, favorites, tags, collections, and documentation.")}</small></div>
        <button
          className={`settings-switch ${status.organizationWrites ? "is-on" : ""}`}
          role="switch"
          aria-checked={status.organizationWrites}
          disabled={writeToggle.isPending}
          onClick={() => writeToggle.mutate(!status.organizationWrites)}
        ><span /></button>
      </div>}
      {(error || status?.lastError) && <div className="form-error">{error || status?.lastError}</div>}
      {status?.enabled && <div className="companion-actions"><button className="button button--primary" disabled={!status.running} onClick={() => void pairDevice()}><QrCode size={16} />{t("Pair a device")}</button><span><ShieldCheck size={14} />{t("No cloud account or public port")}</span></div>}
      {status?.devices.length ? <div className="companion-devices">
        <div className="settings-subheading"><h3>{t("Paired devices")}</h3><p>{t("Revoke a device at any time. Access stops on its next request.")}</p></div>
        {status.devices.map((device) => <div className="companion-device" key={device.id}>
          <span className="companion-device__icon">{device.platform.toLowerCase().includes("desktop") ? <Laptop size={18} /> : <Smartphone size={18} />}</span>
          <div><strong>{device.name}</strong><small>{device.platform} · {device.lastSeenAt ? t("Last connected {date}", { date: formatDate(device.lastSeenAt) }) : t("Not connected yet")}</small></div>
          <button className="icon-button" onClick={() => void revoke(device.id, device.name)} aria-label={t("Revoke {name}", { name: device.name })}><Trash2 size={15} /></button>
        </div>)}
      </div> : status?.enabled && <div className="companion-empty"><Smartphone size={20} /><span><strong>{t("No paired devices")}</strong><small>{t("Pair your phone to start browsing this library.")}</small></span></div>}
      <p className="settings-footnote">{t("Volum listens only on this computer’s Tailscale address. The desktop app must be running; nothing is exposed on your regular Wi-Fi or the public internet.")}</p>
      {pairing && <PairingDialog pairing={pairing} onClose={() => setPairing(undefined)} />}
    </section>
  );
}
