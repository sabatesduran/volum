import { getVersion } from "@tauri-apps/api/app";
import { openAppSettings } from "@tauri-apps/plugin-barcode-scanner";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import {
  Bell,
  CircleAlert,
  CircleCheck,
  CloudDownload,
  HardDrive,
  KeyRound,
  Languages,
  LoaderCircle,
  Sparkles,
  ShieldCheck,
  Trash2,
  Unplug,
  UploadCloud,
  Wifi,
} from "lucide-react";
import { useEffect, useState } from "react";
import { Brand } from "../../components/Brand";
import { PullToRefresh } from "../../components/PullToRefresh";
import { saveCredential } from "../../lib/credentials";
import { openLanguageSettings } from "../../lib/appLocale";
import { formatBytes } from "../../lib/format";
import { isDemoCredential } from "../../lib/demo";
import { languageDefinition, localizedError, t, useLanguage } from "../../lib/i18n";
import { clearMediaCache, mediaCacheStats } from "../../lib/mediaCache";
import { completionNotificationsEnabled, setCompletionNotifications } from "../../lib/notifications";
import { clearOfflineProjects, listOfflineManifests } from "../../lib/offline";
import { normalizeServerUrl, requestJson, verifyServerIdentity } from "../../lib/transport";
import type { Credential, ServerStatus } from "../../types";

export function SettingsScreen({
  credential,
  status,
  onCredentialChange,
  onRefresh,
  onUnpair,
}: {
  credential: Credential;
  status?: ServerStatus;
  onCredentialChange: (credential: Credential) => void;
  onRefresh: () => Promise<unknown>;
  onUnpair: () => void;
}) {
  const queryClient = useQueryClient();
  const [address, setAddress] = useState(credential.baseUrl);
  const [savingAddress, setSavingAddress] = useState(false);
  const [addressError, setAddressError] = useState("");
  const [testingConnection, setTestingConnection] = useState(false);
  const [connectionTest, setConnectionTest] = useState<{ success: boolean; message: string }>();
  const [permissionError, setPermissionError] = useState("");
  const [languageSettingsError, setLanguageSettingsError] = useState("");
  const [clearingCache, setClearingCache] = useState(false);
  const [clearingMedia, setClearingMedia] = useState(false);
  const [completionAlerts, setCompletionAlerts] = useState(completionNotificationsEnabled);
  const [notificationBusy, setNotificationBusy] = useState(false);
  const [appVersion, setAppVersion] = useState("1.0.0");
  const language = useLanguage();
  const demo = isDemoCredential(credential);
  const offline = useQuery({
    queryKey: ["offline-manifests", credential.serverId],
    queryFn: () => listOfflineManifests(credential.serverId),
    staleTime: Infinity,
  });
  const media = useQuery({
    queryKey: ["media-cache", credential.serverId],
    queryFn: () => mediaCacheStats(credential.serverId),
    staleTime: 5_000,
  });

  useEffect(() => setAddress(credential.baseUrl), [credential.baseUrl]);
  useEffect(() => { void getVersion().then(setAppVersion).catch(() => undefined); }, []);

  const saveAddress = async () => {
    setSavingAddress(true);
    setAddressError("");
    setConnectionTest(undefined);
    try {
      const candidate = normalizeServerUrl(address);
      if (candidate.startsWith("http://") && candidate !== credential.baseUrl && !window.confirm(t("This private HTTP connection relies on your network or VPN for encryption. Continue?"))) return;
      const baseUrl = await verifyServerIdentity(candidate, credential.serverId);
      const next = { ...credential, baseUrl };
      const nextStatus = await requestJson<ServerStatus>(next, "/api/v1/status");
      if (nextStatus.serverId !== credential.serverId) {
        throw new Error(t("That address belongs to a different Volum Desktop."));
      }
      await saveCredential(next);
      queryClient.clear();
      onCredentialChange(next);
    } catch (reason) {
      setAddressError(localizedError(reason));
    } finally {
      setSavingAddress(false);
    }
  };

  const testConnection = async () => {
    setTestingConnection(true);
    setAddressError("");
    setConnectionTest(undefined);
    try {
      const candidate = normalizeServerUrl(address);
      if (candidate.startsWith("http://") && candidate !== credential.baseUrl && !window.confirm(t("This private HTTP connection relies on your network or VPN for encryption. Continue?"))) return;
      const baseUrl = await verifyServerIdentity(candidate, credential.serverId);
      const nextStatus = await requestJson<ServerStatus>({ ...credential, baseUrl }, "/api/v1/status");
      if (nextStatus.serverId !== credential.serverId) {
        throw new Error(t("That address belongs to a different Volum Desktop."));
      }
      setConnectionTest({
        success: true,
        message: t("Connected to {name}", { name: nextStatus.name }),
      });
    } catch (reason) {
      setConnectionTest({ success: false, message: localizedError(reason) });
    } finally {
      setTestingConnection(false);
    }
  };

  const clearCache = async () => {
    if (!offline.data?.length || !window.confirm(t("Remove {count} offline {projects} from this device?", {
      count: offline.data.length,
      projects: t(offline.data.length === 1 ? "project" : "projects"),
    }))) return;
    setClearingCache(true);
    setAddressError("");
    try {
      await clearOfflineProjects(credential.serverId);
      await offline.refetch();
      queryClient.removeQueries({ queryKey: ["offline-project", credential.serverId] });
    } catch (reason) {
      setAddressError(localizedError(reason));
    } finally {
      setClearingCache(false);
    }
  };

  const clearTransientMedia = async () => {
    if (!media.data?.count || !window.confirm(t(
      "Remove {size} of recently viewed media from this device? Offline projects will remain available.",
      { size: formatBytes(media.data.byteSize) },
    ))) return;
    setClearingMedia(true);
    setAddressError("");
    try {
      await clearMediaCache(credential.serverId);
      await media.refetch();
    } catch (reason) {
      setAddressError(localizedError(reason));
    } finally {
      setClearingMedia(false);
    }
  };

  const toggleCompletionAlerts = async () => {
    setNotificationBusy(true);
    setAddressError("");
    try {
      const enabled = await setCompletionNotifications(!completionAlerts);
      setCompletionAlerts(enabled);
      if (!enabled && !completionAlerts) setAddressError(t("Notification permission was not granted."));
    } catch (reason) {
      setAddressError(localizedError(reason));
    } finally {
      setNotificationBusy(false);
    }
  };

  const openDeviceSettings = async () => {
    setPermissionError("");
    try {
      await openAppSettings();
    } catch (reason) {
      setPermissionError(localizedError(reason));
    }
  };

  const openDeviceLanguageSettings = async () => {
    setLanguageSettingsError("");
    try {
      await openLanguageSettings();
    } catch (reason) {
      setLanguageSettingsError(localizedError(reason));
    }
  };

  return <main className="library-screen">
    <PullToRefresh onRefresh={onRefresh} />
    <header className="mobile-header">
      <div>
        <h1>{t("Settings")}</h1>
        <p>{t("Connection and privacy")}</p>
      </div>
    </header>

    {demo ? <section className="mobile-settings-card demo-settings-card">
      <span className="settings-icon is-demo"><Sparkles /></span>
      <div>
        <strong>{t("Demo library")}</strong>
        <small>{t("Explore sample projects without an account or a connection to Volum Desktop. Changes and uploads are disabled.")}</small>
      </div>
    </section> : <section className="mobile-settings-card">
      <span className="settings-icon is-online"><Wifi /></span>
      <div>
        <strong>{credential.serverName}</strong>
        <small>{status ? `${status.projectCount} ${t("projects")} · ${t("Connected")}` : "Volum Desktop"}</small>
        <label className="server-address">
          <span>{t("Server address")}</span>
          <div>
            <input value={address} disabled={savingAddress || testingConnection} autoCapitalize="off" autoCorrect="off" onChange={(event) => { setAddress(event.target.value); setAddressError(""); setConnectionTest(undefined); }} />
            <button disabled={savingAddress || testingConnection || address.trim() === credential.baseUrl} onClick={() => void saveAddress()}>
              {savingAddress ? <LoaderCircle className="spin" /> : t("Save")}
            </button>
          </div>
          <small>{t("Use a private HTTP origin or any trusted HTTPS origin.")}</small>
        </label>
        <button className="settings-action-button settings-test-button" disabled={savingAddress || testingConnection || !address.trim()} onClick={() => void testConnection()}>
          {testingConnection ? <LoaderCircle className="spin" /> : <Wifi />}{testingConnection ? t("Testing connection…") : t("Test connection")}
        </button>
        {connectionTest && <div className={`connection-test-result ${connectionTest.success ? "is-success" : "is-error"}`} role={connectionTest.success ? "status" : "alert"}>
          {connectionTest.success ? <CircleCheck /> : <CircleAlert />}<span>{connectionTest.message}</span>
        </div>}
        {addressError && <div className="settings-inline-error" role="alert">{addressError}</div>}
      </div>
    </section>}

    <section className="mobile-settings-card">
      <span className="settings-icon"><Languages /></span>
      <div>
        <strong>{t("Language")}</strong>
        <small>{t("Volum follows this language for the mobile interface.")}</small>
        <div className="language-picker"><strong>{languageDefinition(language).nativeName}</strong></div>
        {languageSettingsError && <div className="settings-inline-error" role="alert">{languageSettingsError}</div>}
        <button className="settings-action-button" onClick={() => void openDeviceLanguageSettings()}>{t("Change in device settings")}</button>
      </div>
    </section>

    <section className="mobile-settings-card">
      <span className="settings-icon"><Bell /></span>
      <div>
        <strong>{t("Completion alerts")}</strong>
        <small>{t("Get a local alert when a file, photo, or offline pack finishes while Volum is open.")}</small>
        <button className={`settings-toggle ${completionAlerts ? "is-active" : ""}`} disabled={notificationBusy} role="switch" aria-checked={completionAlerts} onClick={() => void toggleCompletionAlerts()}>
          <span />{notificationBusy ? t("Checking permission…") : completionAlerts ? t("On") : t("Off")}
        </button>
      </div>
    </section>

    <section className="mobile-settings-card">
      <span className="settings-icon"><KeyRound /></span>
      <div>
        <strong>{t("App permissions")}</strong>
        <small>{t("Manage camera and notification access in your device settings.")}</small>
        {permissionError && <div className="settings-inline-error" role="alert">{permissionError}</div>}
        <button className="settings-action-button" onClick={() => void openDeviceSettings()}>{t("Open device settings")}</button>
      </div>
    </section>

    <section className="mobile-settings-card">
      <span className="settings-icon"><CloudDownload /></span>
      <div>
        <strong>{t("Offline storage")}</strong>
        <small>{offline.data?.length
          ? `${offline.data.length} ${t(offline.data.length === 1 ? "project" : "projects")} · ${formatBytes(offline.data.reduce((total, item) => total + item.byteSize, 0))}`
          : t("No project snapshots are stored on this device.")}</small>
        <button className="settings-cache-button" disabled={clearingCache || !offline.data?.length} onClick={() => void clearCache()}>
          {clearingCache ? <LoaderCircle className="spin" /> : <Trash2 />}{t("Clear offline projects")}
        </button>
      </div>
    </section>

    <section className="mobile-settings-card">
      <span className="settings-icon"><HardDrive /></span>
      <div>
        <strong>{t("Recent media cache")}</strong>
        <small>{media.data?.count
          ? `${media.data.count} ${media.data.count === 1 ? t("item") : t("items")} · ${formatBytes(media.data.byteSize)} · ${t("Automatically limited to 96 MB")}`
          : t("No recently viewed media is cached on this device.")}</small>
        <button className="settings-cache-button" disabled={clearingMedia || !media.data?.count} onClick={() => void clearTransientMedia()}>
          {clearingMedia ? <LoaderCircle className="spin" /> : <Trash2 />}{t("Clear recent media")}
        </button>
      </div>
    </section>

    <button className="unpair-button" onClick={onUnpair}><Unplug size={17} />{t(demo ? "Exit demo" : "Unpair this device")}</button>

    <p className="settings-section-heading">{t("PRIVACY & SAFETY")}</p>
    <section className="mobile-settings-card mobile-settings-card--informational">
      <span className="settings-icon"><ShieldCheck /></span>
      <div>
        <strong>{t("Private by design")}</strong>
        <small>{t("Your token is encrypted on this device. Library requests travel directly through your chosen network without a Volum cloud or relay.")}</small>
      </div>
    </section>
    <section className="mobile-settings-card mobile-settings-card--informational">
      <span className="settings-icon"><UploadCloud /></span>
      <div>
        <strong>{t("Safe file uploads")}</strong>
        <small>{t("Selected 3D files are validated and imported into your managed library. Destructive actions remain on Desktop.")}</small>
      </div>
    </section>

    <footer className="settings-about">
      <Brand />
      <p>v{appVersion} · {t("Protocol")} 1</p>
    </footer>
  </main>;
}
