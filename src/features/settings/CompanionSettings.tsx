import { useEffect, useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Check, Copy, Globe2, Laptop, LoaderCircle, PencilLine, QrCode, ShieldCheck, Smartphone, Trash2, Wifi, WifiOff } from "lucide-react";
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
  const [baseUrl, setBaseUrl] = useState(pairing.baseUrl);

  const editedPayload = useMemo(() => {
    try {
      const url = new URL(baseUrl.trim());
      if (!/^https?:$/.test(url.protocol) || url.username || url.password || url.pathname !== "/" || url.search || url.hash) return "";
      const payload = JSON.parse(pairing.qrPayload) as Record<string, unknown>;
      payload.baseUrl = url.origin;
      payload.transport = url.protocol === "https:" ? "https" : pairing.transport === "tailscale-http" && url.origin === pairing.baseUrl ? "tailscale-http" : "private-http";
      return JSON.stringify(payload);
    } catch {
      return "";
    }
  }, [baseUrl, pairing.baseUrl, pairing.qrPayload, pairing.transport]);

  useEffect(() => {
    let cancelled = false;
    if (!editedPayload) { setQrUrl(""); return; }
    void QRCode.toDataURL(editedPayload, {
      width: 280,
      margin: 2,
      color: { dark: "#191916", light: "#fffefb" },
      errorCorrectionLevel: "M"
    }).then((url) => { if (!cancelled) setQrUrl(url); });
    return () => { cancelled = true; };
  }, [editedPayload]);

  useEffect(() => {
    const update = () => setRemaining(Math.max(0, Math.ceil((Date.parse(pairing.expiresAt) - Date.now()) / 1000)));
    update();
    const timer = window.setInterval(update, 1_000);
    return () => window.clearInterval(timer);
  }, [pairing.expiresAt]);

  const time = `${Math.floor(remaining / 60)}:${String(remaining % 60).padStart(2, "0")}`;
  const copyAddress = async () => {
    await navigator.clipboard.writeText(baseUrl.trim());
    setCopied(true);
    window.setTimeout(() => setCopied(false), 1_500);
  };

  return (
    <Dialog
      title={t("Pair Volum on mobile")}
      subtitle={t("Open Volum on your phone, scan this code, and review the connection address before pairing.")}
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
          <label className="companion-pairing__address"><span>{t("Connection address")}</span><input value={baseUrl} onChange={(event) => setBaseUrl(event.target.value)} autoCapitalize="off" autoCorrect="off" /></label>
          <div><span>{t("Expires in")}</span><strong>{time}</strong></div>
        </div>
        {!editedPayload && <div className="form-error">{t("Enter a complete HTTP or HTTPS server origin.")}</div>}
        <button className="button button--quiet" disabled={!editedPayload} onClick={() => void copyAddress()}>
          {copied ? <Check size={15} /> : <Copy size={15} />}
          {t(copied ? "Copied" : "Copy address")}
        </button>
        <p className="settings-footnote">{t("The QR code contains a one-time secret. It stops working after one successful pairing or five minutes.")}</p>
        <p className="settings-footnote"><ShieldCheck size={14} />{baseUrl.trim().startsWith("https://") ? t("HTTPS protects this connection.") : pairing.transport === "tailscale-http" && baseUrl.trim() === pairing.baseUrl ? t("Tailscale protects this private HTTP connection.") : t("Private HTTP relies on your network or VPN for encryption.")}</p>
      </div>
    </Dialog>
  );
}

export function CompanionSettings() {
  const queryClient = useQueryClient();
  const [pairing, setPairing] = useState<CompanionPairingInfo>();
  const [error, setError] = useState("");
  const [networkMode, setNetworkMode] = useState<"automatic" | "custom">("automatic");
  const [bindAddress, setBindAddress] = useState("");
  const [publicUrl, setPublicUrl] = useState("");
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
  useEffect(() => {
    if (!status) return;
    setNetworkMode(status.networkMode ?? "automatic");
    setBindAddress(status.bindAddress ?? "");
    setPublicUrl(status.publicUrl ?? status.baseUrl ?? "");
  }, [status]);
  const networkMutation = useMutation({
    mutationFn: () => api.setCompanionNetwork(networkMode, networkMode === "custom" ? bindAddress : undefined, networkMode === "custom" ? publicUrl : undefined),
    onSuccess: (next) => { queryClient.setQueryData(["companion-status"], next); setError(next.lastError ?? ""); },
    onError: (reason) => setError(reason instanceof Error ? reason.message : String(reason))
  });
  const runningLabel = useMemo(() => {
    if (isLoading) return t("Checking…");
    if (status?.running) return t("Available on your selected network");
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
        <div><h2>{t("Companion Access")}</h2><p>{t("Browse your Volum library from iPhone, iPad, or Android through Tailscale, another private network, or HTTPS.")}</p></div>
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
        <div><strong>{runningLabel}</strong><small>{status?.running ? t(status.transport === "https" ? "Protected by HTTPS and a Volum device token." : status.transport === "tailscale-http" ? "Protected by Tailscale and a Volum device token." : "Protected by your private network or VPN and a Volum device token.") : t("Turn on access while Volum Desktop is running to connect your phone.")}</small></div>
        {status?.baseUrl && <code>{status.baseUrl}</code>}
      </div>
      <div className="companion-network">
        <div className="settings-subheading"><h3>{t("Connection network")}</h3><p>{t("Use automatic Tailscale detection or advertise another private or HTTPS address.")}</p></div>
        <div className="companion-network__modes" role="radiogroup" aria-label={t("Connection network")}>
          <button className={networkMode === "automatic" ? "is-active" : ""} role="radio" aria-checked={networkMode === "automatic"} onClick={() => setNetworkMode("automatic")}><Wifi size={16} /><span><strong>{t("Automatic")}</strong><small>{t("Detect Tailscale")}</small></span></button>
          <button className={networkMode === "custom" ? "is-active" : ""} role="radio" aria-checked={networkMode === "custom"} onClick={() => setNetworkMode("custom")}><Globe2 size={16} /><span><strong>{t("Custom")}</strong><small>{t("LAN, VPN, or HTTPS")}</small></span></button>
        </div>
        {networkMode === "custom" && <div className="companion-network__fields">
          <label><span>{t("Listen on this private IP")}</span><input value={bindAddress} onChange={(event) => setBindAddress(event.target.value)} placeholder="192.168.1.20" autoCapitalize="off" autoCorrect="off" /></label>
          <label><span>{t("Address clients will use")}</span><input value={publicUrl} onChange={(event) => setPublicUrl(event.target.value)} placeholder="https://volum.example.com" autoCapitalize="off" autoCorrect="off" /></label>
          <p><ShieldCheck size={14} />{publicUrl.trim().startsWith("https://") ? t("HTTPS protects this connection.") : t("Private HTTP relies on your network or VPN for encryption. Public HTTP is blocked.")}</p>
        </div>}
        <button className="button button--quiet" disabled={networkMutation.isPending || (networkMode === "custom" && (!bindAddress.trim() || !publicUrl.trim()))} onClick={() => networkMutation.mutate()}>{networkMutation.isPending ? <LoaderCircle className="spin" /> : <Check size={15} />}{t("Save network settings")}</button>
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
      {status?.enabled && <div className="companion-actions"><button className="button button--primary" disabled={!status.running} onClick={() => void pairDevice()}><QrCode size={16} />{t("Pair a device")}</button><span><ShieldCheck size={14} />{t("No Volum cloud or relay")}</span></div>}
      {status?.devices.length ? <div className="companion-devices">
        <div className="settings-subheading"><h3>{t("Paired devices")}</h3><p>{t("Revoke a device at any time. Access stops on its next request.")}</p></div>
        {status.devices.map((device) => <div className="companion-device" key={device.id}>
          <span className="companion-device__icon">{device.platform.toLowerCase().includes("desktop") ? <Laptop size={18} /> : <Smartphone size={18} />}</span>
          <div><strong>{device.name}</strong><small>{device.platform} · {device.lastSeenAt ? t("Last connected {date}", { date: formatDate(device.lastSeenAt) }) : t("Not connected yet")}</small></div>
          <button className="icon-button" onClick={() => void revoke(device.id, device.name)} aria-label={t("Revoke {name}", { name: device.name })}><Trash2 size={15} /></button>
        </div>)}
      </div> : status?.enabled && <div className="companion-empty"><Smartphone size={20} /><span><strong>{t("No paired devices")}</strong><small>{t("Pair your phone to start browsing this library.")}</small></span></div>}
      <p className="settings-footnote">{t(networkMode === "automatic" ? "Automatic mode listens only on this computer’s Tailscale address." : "Custom mode listens only on the private IP you choose and advertises the separate client address above.")} {t("The desktop app must be running. Public HTTP is always blocked; use HTTPS outside a private network.")}</p>
      {pairing && <PairingDialog pairing={pairing} onClose={() => setPairing(undefined)} />}
    </section>
  );
}
