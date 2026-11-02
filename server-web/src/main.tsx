import { StrictMode, useCallback, useEffect, useMemo, useState } from "react";
import { createRoot } from "react-dom/client";
import {
  Check, CircleAlert, Clock3, Copy, DatabaseBackup, KeyRound, Laptop, LoaderCircle, LogOut,
  Plus, QrCode, RefreshCw, Server, ShieldCheck, Smartphone, Trash2, X,
} from "lucide-react";
import QRCode from "qrcode";
import {
  bootstrap, createMigrationPairing, createPairing, listDevices, login, logout, revokeDevice,
  type Bootstrap, type Device, type Pairing, WebApiError,
} from "./api";
import { clearCredential, loadCredential, saveCredential, type WebCredential } from "./credential";
import "./styles.css";

function browserName() {
  if (/iphone|ipad/i.test(navigator.userAgent)) return "Safari on iPhone";
  if (/android/i.test(navigator.userAgent)) return "Android browser";
  if (/firefox/i.test(navigator.userAgent)) return "Firefox browser";
  if (/edg/i.test(navigator.userAgent)) return "Edge browser";
  if (/chrome/i.test(navigator.userAgent)) return "Chrome browser";
  if (/safari/i.test(navigator.userAgent)) return "Safari browser";
  return "Web browser";
}

function friendlyDate(value?: string) {
  if (!value) return "Never connected";
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? "Unknown" : new Intl.DateTimeFormat(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(date);
}

function errorMessage(reason: unknown) {
  return reason instanceof Error ? reason.message : String(reason);
}

function Brand() {
  return <div className="brand" aria-label="Volum"><span className="brand__mark"><Server /></span><strong>volum<span>.</span></strong></div>;
}

function LoadingScreen() {
  return <main className="center-screen"><Brand /><LoaderCircle className="spin" aria-label="Loading" /></main>;
}

function SetupRequired({ host }: { host?: Bootstrap }) {
  return <main className="auth-layout">
    <section className="auth-intro"><Brand /><div><h1>Finish server setup.</h1><p>Set one administrator password in UGREEN, then restart the Volum container.</p></div></section>
    <section className="auth-panel">
      <span className="auth-panel__icon"><KeyRound /></span>
      <h2>Administrator password required</h2>
      <p>Add this environment variable to the container. It must contain at least 12 characters.</p>
      <code>VOLUM_ADMIN_PASSWORD</code>
      <p className="quiet">Volum stores an Argon2id hash under <code>/config</code>. The environment value can be removed after the first successful restart.</p>
      {host?.publicUrl && <small>Server address: {host.publicUrl}</small>}
    </section>
  </main>;
}

function LoginScreen({ host, onLogin }: { host: Bootstrap; onLogin: (credential: WebCredential) => void }) {
  const [password, setPassword] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    setBusy(true); setError("");
    try {
      const credential = await login(password, browserName());
      await saveCredential(credential);
      onLogin(credential);
    } catch (reason) {
      setError(errorMessage(reason));
    } finally { setBusy(false); }
  };
  return <main className="auth-layout">
    <section className="auth-intro">
      <Brand />
      <div><h1>Your library lives here.</h1><p>Manage trusted devices without opening Docker logs.</p></div>
      <div className={`transport-note ${host.secureTransport ? "is-secure" : ""}`}><ShieldCheck /><span><strong>{host.secureTransport ? "Protected by HTTPS" : "Private network connection"}</strong><small>{host.publicUrl}</small></span></div>
    </section>
    <form className="auth-panel" onSubmit={(event) => void submit(event)}>
      <span className="auth-panel__icon"><KeyRound /></span>
      <h2>Sign in to {host.serverName}</h2>
      <p>Use the administrator password configured on this server.</p>
      <label><span>Password</span><input autoFocus type="password" autoComplete="current-password" value={password} onChange={(event) => setPassword(event.target.value)} /></label>
      {error && <div className="alert" role="alert"><CircleAlert />{error}</div>}
      <button className="primary-button" disabled={busy || !password}>{busy ? <LoaderCircle className="spin" /> : <ShieldCheck />}Sign in</button>
    </form>
  </main>;
}

function PairingDialog({ pairing, qrUrl, paired, purpose, onCopy, onClose, onRefresh }: {
  pairing: Pairing;
  qrUrl: string;
  paired: boolean;
  purpose: "device" | "migration";
  onCopy: () => void;
  onClose: () => void;
  onRefresh: () => void;
}) {
  const [remaining, setRemaining] = useState(() => Math.max(0, Math.ceil((Date.parse(pairing.expiresAt) - Date.now()) / 1000)));
  useEffect(() => {
    const update = () => setRemaining(Math.max(0, Math.ceil((Date.parse(pairing.expiresAt) - Date.now()) / 1000)));
    update();
    const timer = window.setInterval(update, 1000);
    return () => window.clearInterval(timer);
  }, [pairing.expiresAt]);
  const expired = remaining <= 0;
  const minutes = Math.floor(remaining / 60);
  const seconds = String(remaining % 60).padStart(2, "0");
  return <div className="dialog-scrim" role="presentation" onClick={onClose}>
    <section className="pairing-dialog" role="dialog" aria-modal="true" aria-labelledby="pair-title" onClick={(event) => event.stopPropagation()}>
      <header><div><h2 id="pair-title">{purpose === "migration" ? "Move a Desktop library" : "Pair a device"}</h2><p>{purpose === "migration" ? "Copy this one-time code into Volum Desktop’s Move library to NAS assistant." : "Open Volum on your phone and scan this code."}</p></div><button className="icon-button" aria-label="Close" onClick={onClose}><X /></button></header>
      {paired ? <div className="pairing-result"><span><Check /></span><h3>Device connected</h3><p>The one-time code has been consumed.</p></div> : expired ? <div className="pairing-result is-expired"><span><Clock3 /></span><h3>Code expired</h3><p>Create another one-time code to continue.</p><button className="primary-button" onClick={onRefresh}><RefreshCw />Create new code</button></div> : <>
        {purpose === "device" && <div className="qr-stage">{qrUrl ? <img src={qrUrl} alt="Volum pairing QR code" /> : <LoaderCircle className="spin" />}</div>}
        <div className="pairing-meta"><span><Clock3 />Expires in {minutes}:{seconds}</span><span><ShieldCheck />Works once</span></div>
        <button className="secondary-button" onClick={onCopy}><Copy />Copy pairing data</button>
        <p className="pairing-help">{purpose === "migration" ? "The NAS must not contain projects. Volum verifies the complete transfer before this server becomes canonical." : "The phone may review the server address before connecting. Keep this code private until it expires."}</p>
      </>}
    </section>
  </div>;
}

function DeviceIcon({ device }: { device: Device }) {
  return device.role === "admin" || /web|desktop/i.test(device.platform) ? <Laptop /> : <Smartphone />;
}

function DevicesApp({ host, credential, onSignedOut }: { host: Bootstrap; credential: WebCredential; onSignedOut: () => void }) {
  const [devices, setDevices] = useState<Device[]>([]);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState("");
  const [error, setError] = useState("");
  const [pairing, setPairing] = useState<Pairing>();
  const [pairingPurpose, setPairingPurpose] = useState<"device" | "migration">("device");
  const [qrUrl, setQrUrl] = useState("");
  const [pairingClientIds, setPairingClientIds] = useState<string[]>([]);
  const [copied, setCopied] = useState(false);

  const handleUnauthorized = useCallback(async (reason: unknown) => {
    if (reason instanceof WebApiError && reason.status === 401) {
      await clearCredential().catch(() => undefined);
      onSignedOut();
      return true;
    }
    return false;
  }, [onSignedOut]);

  const refresh = useCallback(async (quiet = false) => {
    if (!quiet) setLoading(true);
    try {
      const next = await listDevices(credential);
      setDevices(next);
      setError("");
    } catch (reason) {
      if (!await handleUnauthorized(reason)) setError(errorMessage(reason));
    } finally { if (!quiet) setLoading(false); }
  }, [credential, handleUnauthorized]);

  useEffect(() => { void refresh(); }, [refresh]);
  useEffect(() => {
    if (!pairing) return;
    const timer = window.setInterval(() => void refresh(true), 3000);
    return () => window.clearInterval(timer);
  }, [pairing, refresh]);
  useEffect(() => {
    if (!pairing) { setQrUrl(""); return; }
    let active = true;
    void QRCode.toDataURL(pairing.qrPayload, { width: 420, margin: 2, errorCorrectionLevel: "M" })
      .then((url) => { if (active) setQrUrl(url); })
      .catch((reason) => { if (active) setError(errorMessage(reason)); });
    return () => { active = false; };
  }, [pairing]);
  const paired = Boolean(pairing && devices.some(
    (device) => device.role !== "admin" && !pairingClientIds.includes(device.id),
  ));

  const beginPairing = async (purpose: "device" | "migration" = "device") => {
    setBusy("pair"); setError(""); setCopied(false);
    try {
      const next = await (purpose === "migration" ? createMigrationPairing(credential) : createPairing(credential));
      setPairingPurpose(purpose);
      setPairingClientIds(devices.filter((device) => device.role !== "admin").map((device) => device.id));
      setPairing(next);
    } catch (reason) {
      if (!await handleUnauthorized(reason)) setError(errorMessage(reason));
    } finally { setBusy(""); }
  };
  const remove = async (device: Device) => {
    if (!window.confirm(`Revoke access for “${device.name}”?`)) return;
    setBusy(device.id); setError("");
    try {
      await revokeDevice(credential, device.id);
      if (device.id === credential.deviceId) {
        await clearCredential(); onSignedOut(); return;
      }
      await refresh(true);
    } catch (reason) {
      if (!await handleUnauthorized(reason)) setError(errorMessage(reason));
    } finally { setBusy(""); }
  };
  const signOut = async () => {
    setBusy("logout");
    try { await logout(credential); } catch { /* local sign-out still completes */ }
    await clearCredential().catch(() => undefined);
    onSignedOut();
  };
  const copyPairing = async () => {
    if (!pairing) return;
    try { await navigator.clipboard.writeText(pairing.qrPayload); setCopied(true); }
    catch { setError("The browser could not copy the pairing data."); }
  };

  const clients = useMemo(() => devices.filter((device) => device.role !== "admin"), [devices]);
  const browsers = useMemo(() => devices.filter((device) => device.role === "admin"), [devices]);
  return <div className="app-shell">
    <aside className="sidebar"><Brand /><nav><button className="is-active"><Smartphone />Devices</button></nav><div className="server-chip"><span><Server /></span><div><strong>{host.serverName}</strong><small>{host.secureTransport ? "HTTPS" : "Private network"}</small></div></div></aside>
    <main className="devices-page">
      <header className="page-header"><div><h1>Trusted devices</h1><p>Pair a phone or revoke access without touching the container logs.</p></div><div className="header-actions"><button className="secondary-button" disabled={loading} onClick={() => void refresh()}><RefreshCw className={loading ? "spin" : ""} />Refresh</button><button className="primary-button" disabled={busy === "pair"} onClick={() => void beginPairing()}>{busy === "pair" ? <LoaderCircle className="spin" /> : <Plus />}Pair device</button></div></header>
      {error && <div className="alert page-alert" role="alert"><CircleAlert />{error}</div>}
       <section className="device-panel" aria-labelledby="paired-devices"><header><div><h2 id="paired-devices">Paired devices</h2><p>{clients.length ? `${clients.length} phone or desktop ${clients.length === 1 ? "client" : "clients"}` : "No companion devices yet"}</p></div><span>{clients.length}</span></header>
        {loading ? <div className="panel-state"><LoaderCircle className="spin" />Loading devices</div> : clients.length ? <div className="device-list">{clients.map((device) => <article className="device-row" key={device.id}><span className="device-icon"><DeviceIcon device={device} /></span><div><strong>{device.name}{device.role === "migration" && <em>Transfer only</em>}</strong><small>{device.platform} · Paired {friendlyDate(device.pairedAt)}</small><small>Last seen {friendlyDate(device.lastSeenAt)}</small></div><button className="revoke-button" disabled={busy === device.id} onClick={() => void remove(device)}>{busy === device.id ? <LoaderCircle className="spin" /> : <Trash2 />}<span>Revoke</span></button></article>)}</div> : <div className="empty-state"><span><QrCode /></span><h3>Pair your first device</h3><p>Generate a private, one-time QR code and scan it in Volum on your phone.</p><button className="primary-button" onClick={() => void beginPairing()}><QrCode />Show pairing code</button></div>}
      </section>
      <section className="migration-panel" aria-labelledby="desktop-migration"><span><DatabaseBackup /></span><div><h2 id="desktop-migration">Move a Desktop library here</h2><p>Generate a one-time code, then paste it into <strong>Desktop → Settings → Connection → Move this library to a NAS</strong>. The transfer is resumable and this NAS refuses to merge an existing library.</p></div><button className="secondary-button" disabled={busy === "pair"} onClick={() => void beginPairing("migration")}>{busy === "pair" ? <LoaderCircle className="spin" /> : <DatabaseBackup />}Create transfer code</button></section>
      <section className="browser-panel" aria-labelledby="administrator-browsers">
        <header><div><ShieldCheck /><span><h2 id="administrator-browsers">Administrator browsers</h2><small>{browsers.length} active {browsers.length === 1 ? "browser" : "browsers"}. Each has a separate revocable token.</small></span></div></header>
        <div className="browser-list">{browsers.map((device) => {
          const current = device.id === credential.deviceId;
          return <article className="browser-row" key={device.id}><span className="device-icon"><Laptop /></span><div><strong>{device.name}{current && <em>Current</em>}</strong><small>Paired {friendlyDate(device.pairedAt)} · Last active {friendlyDate(device.lastSeenAt)}</small></div><button className={current ? "text-button" : "revoke-button"} disabled={busy === device.id || busy === "logout"} onClick={() => current ? void signOut() : void remove(device)}>{busy === device.id || (current && busy === "logout") ? <LoaderCircle className="spin" /> : current ? <LogOut /> : <Trash2 />}<span>{current ? "Sign out" : "Revoke"}</span></button></article>;
        })}</div>
      </section>
      <footer>Volum protocol {host.protocolVersion} · {host.publicUrl}</footer>
    </main>
    {pairing && <PairingDialog pairing={pairing} qrUrl={qrUrl} paired={paired} purpose={pairingPurpose} onCopy={() => void copyPairing()} onClose={() => setPairing(undefined)} onRefresh={() => void beginPairing(pairingPurpose)} />}
    {copied && <div className="toast" role="status"><Check />Pairing data copied</div>}
  </div>;
}

function App() {
  const [host, setHost] = useState<Bootstrap>();
  const [credential, setCredential] = useState<WebCredential>();
  const [loading, setLoading] = useState(true);
  const [fatalError, setFatalError] = useState("");
  useEffect(() => {
    void Promise.all([bootstrap(), loadCredential().catch(() => undefined)])
      .then(async ([nextHost, saved]) => {
        setHost(nextHost);
        if (saved && saved.serverId !== nextHost.serverId) {
          await clearCredential().catch(() => undefined);
          setCredential(undefined);
        } else {
          setCredential(saved);
        }
      })
      .catch((reason) => setFatalError(errorMessage(reason)))
      .finally(() => setLoading(false));
  }, []);
  if (loading) return <LoadingScreen />;
  if (fatalError || !host) return <main className="center-screen"><CircleAlert /><h1>Volum server is unavailable</h1><p>{fatalError || "The server did not return its configuration."}</p><button className="secondary-button" onClick={() => window.location.reload()}><RefreshCw />Try again</button></main>;
  if (!host.configured) return <SetupRequired host={host} />;
  if (!credential) return <LoginScreen host={host} onLogin={setCredential} />;
  return <DevicesApp host={host} credential={credential} onSignedOut={() => setCredential(undefined)} />;
}

createRoot(document.getElementById("root")!).render(<StrictMode><App /></StrictMode>);
