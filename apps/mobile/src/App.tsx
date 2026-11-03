import { lazy, Suspense, useCallback, useEffect, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { cancel, Format, requestPermissions, scan } from "@tauri-apps/plugin-barcode-scanner";
import { open } from "@tauri-apps/plugin-dialog";
import {
  Boxes, Camera, CircleAlert, Heart, LibraryBig, LoaderCircle, QrCode, Settings2,
  ShieldCheck, Wifi, X,
} from "lucide-react";
import volumIcon from "./assets/volum-icon-transparent.png";
import { Brand } from "./components/Brand";
import { QrScannerOverlay } from "./components/QrScannerOverlay";
import type { Tab } from "./features/library/LibraryScreen";
import { clearCredential, loadCredential, prepareCredentialStore, saveCredential } from "./lib/credentials";
import { clearOfflineProjects } from "./lib/offline";
import { clearMediaCache } from "./lib/mediaCache";
import { demoCredential, isDemoCredential } from "./lib/demo";
import { localizedError, t, useLanguage } from "./lib/i18n";
import {
  removeLocalStl,
  stageOpenedStl,
  stageSelectedStl,
  takeOpenedStls,
  type LocalStlFile,
} from "./lib/localStl";
import { filenameFromFilePath, pairWithDesktop, pairingPayloadWithAddress, parsePairingPayload, requestJson, transportForServerUrl, verifyServerIdentity } from "./lib/transport";
import type { Credential, PairingPayload, ServerStatus } from "./types";

const LibraryScreen = lazy(() => import("./features/library/LibraryScreen").then((module) => ({ default: module.LibraryScreen })));
const CollectionsScreen = lazy(() => import("./features/organization/CollectionsScreen").then((module) => ({ default: module.CollectionsScreen })));
const ProjectDetailScreen = lazy(() => import("./features/project/ProjectDetailScreen").then((module) => ({ default: module.ProjectDetailScreen })));
const SettingsScreen = lazy(() => import("./features/settings/SettingsScreen").then((module) => ({ default: module.SettingsScreen })));
const LocalStlPreviewScreen = lazy(() => import("./features/upload/LocalStlPreviewScreen").then((module) => ({ default: module.LocalStlPreviewScreen })));
const UploadScreen = lazy(() => import("./features/upload/UploadScreen").then((module) => ({ default: module.UploadScreen })));

function ScreenFallback() {
  return <main className="launch-screen" role="status"><LoaderCircle className="spin" /><span>{t("Opening…")}</span></main>;
}

export function PairingScreen({ onPaired, onDemo, onCancel }: { onPaired: (credential: Credential) => void; onDemo: () => void; onCancel?: () => void }) {
  const [manual, setManual] = useState(false);
  const [payloadText, setPayloadText] = useState("");
  const [busy, setBusy] = useState(false);
  const [scanning, setScanning] = useState(false);
  const [error, setError] = useState("");
  const [pendingPayload, setPendingPayload] = useState<PairingPayload>();
  const [serverAddress, setServerAddress] = useState("");
  const [acceptPrivateHttp, setAcceptPrivateHttp] = useState(false);
  const scanCanceled = useRef(false);

  const prepareConnection = (value: string) => {
    try {
      const payload = parsePairingPayload(value);
      setPendingPayload(payload);
      setServerAddress(payload.baseUrl);
      setAcceptPrivateHttp(false);
      setError("");
    } catch (reason) {
      setError(localizedError(reason));
    }
  };
  const connect = async (
    payloadToConnect = pendingPayload,
    address = serverAddress,
    showReviewOnFailure = false,
  ) => {
    if (!payloadToConnect) return;
    setBusy(true); setError("");
    try {
      // Open secure storage before sending the one-use secret. A local vault
      // failure must not consume a pairing code on Volum Desktop.
      await prepareCredentialStore();
      const payload = pairingPayloadWithAddress(payloadToConnect, address);
      const credential = await pairWithDesktop(payload);
      await saveCredential(credential);
      onPaired(credential);
    } catch (reason) {
      if (showReviewOnFailure) {
        setPendingPayload(payloadToConnect);
        setServerAddress(address);
        setAcceptPrivateHttp(false);
      }
      setError(localizedError(reason));
    } finally { setBusy(false); }
  };
  const scanCode = async () => {
    scanCanceled.current = false;
    setScanning(true); setError("");
    try {
      const permission = await requestPermissions();
      if (permission !== "granted") throw new Error(t("Camera access is needed to scan the pairing code."));
      if (scanCanceled.current) return;
      const result = await scan({ formats: [Format.QRCode], cameraDirection: "back", windowed: true });
      if (scanCanceled.current) return;
      setScanning(false);
      const payload = parsePairingPayload(result.content);
      if (payload.transport === "private-http") {
        // Keep the explicit trust confirmation for unencrypted private HTTP.
        prepareConnection(result.content);
      } else {
        await connect(payload, payload.baseUrl, true);
      }
    } catch (reason) {
      const message = reason instanceof Error ? reason.message : String(reason);
      if (!scanCanceled.current && !/cancel/i.test(message)) setError(localizedError(reason));
    } finally { setScanning(false); }
  };
  const cancelPairingScan = async () => {
    scanCanceled.current = true;
    setScanning(false);
    await cancel().catch(() => undefined);
  };

  return <main className="pairing-screen">
    {onCancel && <button className="pairing-cancel round-button" aria-label={t("Back to STL preview")} onClick={onCancel}><X /></button>}
    <section className="pairing-intro">
      <div className="pairing-icon-stage" aria-hidden="true"><span /><span /><img src={volumIcon} alt="" /></div>
      <div className="eyebrow">{t("Your private 3D library")}</div>
      <h1>{t("From desktop")}<br />{t("to workbench.")}</h1>
      <p>{t("Keep project files, photos, parts, and build steps close while you print.")}</p>
    </section>
    <section className="pairing-card">
      <div className="pairing-card__heading"><span><QrCode /></span><div><strong>{t("Connect to Volum Desktop")}</strong><p>{t("Open Settings → Companion Access on your Mac or PC.")}</p></div></div>
      {pendingPayload ? <div className="pairing-review">
        <div className="pairing-review__server"><strong>{pendingPayload.serverName}</strong><span>{t("Verify the address before connecting")}</span></div>
        <label><span>{t("Server address")}</span><input value={serverAddress} disabled={busy} autoCapitalize="off" autoCorrect="off" onChange={(event) => { setServerAddress(event.target.value); setAcceptPrivateHttp(false); }} /></label>
        {(() => {
          try {
            const transport = transportForServerUrl(serverAddress, pendingPayload);
            if (transport === "https") return <div className="connection-security is-secure"><ShieldCheck />{t("HTTPS protects this connection.")}</div>;
            if (transport === "tailscale-http") return <div className="connection-security is-secure"><ShieldCheck />{t("Tailscale protects this private connection.")}</div>;
            return <label className="private-http-confirm"><input type="checkbox" checked={acceptPrivateHttp} onChange={(event) => setAcceptPrivateHttp(event.target.checked)} /><span><strong>{t("Trust this private HTTP connection")}</strong><small>{t("Volum will rely on your private network or VPN for encryption. Public HTTP is blocked.")}</small></span></label>;
          } catch { return <div className="connection-security is-error"><CircleAlert />{t("Enter a valid server origin.")}</div>; }
        })()}
        <button className="button button--primary button--large" disabled={busy || (() => { try { return transportForServerUrl(serverAddress, pendingPayload) === "private-http" && !acceptPrivateHttp; } catch { return true; } })()} onClick={() => void connect()}>{busy ? <LoaderCircle className="spin" /> : <Wifi />}{t("Verify and connect")}</button>
        <button className="text-button" disabled={busy} onClick={() => setPendingPayload(undefined)}>{t("Scan a different code")}</button>
      </div> : manual ? <div className="manual-pairing">
        <label><span>{t("Pairing data")}</span><textarea value={payloadText} onChange={(event) => setPayloadText(event.target.value)} placeholder={t("Paste from Volum Desktop")} autoCapitalize="off" autoCorrect="off" /></label>
        <button className="button button--primary button--large" disabled={busy || !payloadText.trim()} onClick={() => prepareConnection(payloadText)}><Wifi />{t("Review connection")}</button>
        <button className="text-button" onClick={() => setManual(false)}>{t("Use the camera instead")}</button>
      </div> : <div className="pairing-actions">
        <button className="button button--primary button--large" disabled={busy || scanning} onClick={() => void scanCode()}>{busy || scanning ? <LoaderCircle className="spin" /> : <QrCode />}{t(busy ? "Connecting…" : "Scan desktop code")}</button>
        <button className="text-button" disabled={scanning} onClick={() => setManual(true)}>{t("Enter pairing data manually")}</button>
        <button id="store-demo" className="demo-button" disabled={scanning} onClick={onDemo}><LibraryBig />{t("Explore the demo library")}</button>
      </div>}
      {error && <div className="error-banner" role="alert"><CircleAlert size={17} /><span>{error}</span></div>}
      <footer><ShieldCheck size={15} /><span><strong>{t("Private by design")}</strong>{t("No account, cloud, or public relay.")}</span></footer>
    </section>
    {scanning && <QrScannerOverlay title={t("Scan the pairing code")} instructions={t("Point the camera at the code shown in Volum Desktop. You can cancel at any time.")} onCancel={() => void cancelPairingScan()} />}
  </main>;
}

export function CompanionApp({ credential, initialProjectId, onInitialProjectOpened, onCredentialChange, onUnpair, onPreviewStl }: { credential: Credential; initialProjectId?: string; onInitialProjectOpened?: () => void; onCredentialChange: (credential: Credential) => void; onUnpair: () => void; onPreviewStl: (filePath: string, filename: string) => Promise<void> }) {
  const queryClient = useQueryClient();
  const observedRevision = useRef("");
  const [tab, setTab] = useState<Tab>("library");
  const [projectId, setProjectId] = useState<string | undefined>(initialProjectId);
  const [collectionId, setCollectionId] = useState<string>();
  const status = useQuery({
    queryKey: ["status", credential.baseUrl],
    queryFn: () => requestJson<ServerStatus>(credential, "/api/v1/status"),
    refetchInterval: () => document.visibilityState === "visible" ? 15_000 : false,
    refetchOnReconnect: true,
  });
  useEffect(() => {
    const next = status.data?.libraryRevision;
    if (!next) return;
    if (observedRevision.current && observedRevision.current !== next) {
      void queryClient.invalidateQueries({ predicate: (query) => query.queryKey[0] !== "status" && query.queryKey[0] !== "offline-project" && query.queryKey[0] !== "offline-manifests" });
    }
    observedRevision.current = next;
  }, [queryClient, status.data?.libraryRevision]);
  useEffect(() => {
    if (initialProjectId) onInitialProjectOpened?.();
  }, [initialProjectId, onInitialProjectOpened]);
  const canWrite = Boolean(status.data?.capabilities.organizationWrites);
  if (projectId) return <Suspense fallback={<ScreenFallback />}><ProjectDetailScreen key={projectId} credential={credential} id={projectId} canWrite={canWrite} onBack={() => setProjectId(undefined)} onOpen={setProjectId} /></Suspense>;
  const chooseTab = (next: Tab) => { setTab(next); if (next !== "library") setCollectionId(undefined); };
  return <div className={`app-shell ${isDemoCredential(credential) ? "is-demo" : ""}`}>
    <Suspense fallback={<ScreenFallback />}>{tab === "collections" ? <CollectionsScreen credential={credential} canWrite={canWrite} onSelect={(id) => { setCollectionId(id); setTab("library"); }} onOpen={setProjectId} /> : tab === "upload" ? <UploadScreen credential={credential} photosEnabled={Boolean(status.data?.capabilities.photoUploads)} onOpen={setProjectId} onPreviewStl={onPreviewStl} /> : tab === "settings" ? <SettingsScreen credential={credential} status={status.data} onCredentialChange={onCredentialChange} onRefresh={() => status.refetch()} onUnpair={onUnpair} /> : <LibraryScreen credential={credential} tab={tab} selectedCollection={collectionId} onCollection={setCollectionId} onOpen={setProjectId} />}</Suspense>
    <nav className="bottom-nav">{([
      ["library", "Library", LibraryBig], ["favorites", "Favorites", Heart], ["upload", "Capture", Camera], ["collections", "Collections", Boxes], ["settings", "Settings", Settings2]
    ] as Array<[Tab, string, typeof LibraryBig]>).map(([id, label, Icon]) => <button id={`nav-${id}`} key={id} className={tab === id ? "is-active" : ""} onClick={() => chooseTab(id)}><Icon size={20} fill={id === "favorites" && tab === id ? "currentColor" : "none"} /><span>{t(label)}</span></button>)}</nav>
  </div>;
}

export function App() {
  useLanguage();
  const queryClient = useQueryClient();
  const [credential, setCredential] = useState<Credential>();
  const [unverifiedCredential, setUnverifiedCredential] = useState<Credential>();
  const [verificationError, setVerificationError] = useState("");
  const [loading, setLoading] = useState(true);
  const [localFile, setLocalFile] = useState<LocalStlFile>();
  const localFileRef = useRef<LocalStlFile | undefined>(undefined);
  const [stagingLocalFile, setStagingLocalFile] = useState(false);
  const [incomingError, setIncomingError] = useState("");
  const [pairingForLocalFile, setPairingForLocalFile] = useState(false);
  const [resumeLocalSave, setResumeLocalSave] = useState(false);
  const [initialProjectId, setInitialProjectId] = useState<string>();
  const drainRunning = useRef(false);
  const verifySavedCredential = useCallback(async (saved: Credential) => {
    setLoading(true); setVerificationError("");
    try {
      const baseUrl = isDemoCredential(saved)
        ? saved.baseUrl
        : await verifyServerIdentity(saved.baseUrl, saved.serverId);
      const verified = { ...saved, baseUrl };
      setUnverifiedCredential(undefined);
      setCredential(verified);
    } catch (reason) {
      setCredential(undefined);
      setUnverifiedCredential(saved);
      setVerificationError(localizedError(reason));
    } finally { setLoading(false); }
  }, []);
  useEffect(() => {
    void loadCredential()
      .then((saved) => saved ? verifySavedCredential(saved) : undefined)
      .catch((reason) => setVerificationError(localizedError(reason)))
      .finally(() => setLoading(false));
  }, [verifySavedCredential]);
  const forgetUnverifiedCredential = async () => {
    const saved = unverifiedCredential;
    if (saved) {
      await Promise.allSettled([
        clearOfflineProjects(saved.serverId),
        clearMediaCache(saved.serverId),
      ]);
    }
    await clearCredential();
    setUnverifiedCredential(undefined);
    setVerificationError("");
  };
  const replaceLocalFile = useCallback((next: LocalStlFile) => {
    const previous = localFileRef.current;
    localFileRef.current = next;
    setLocalFile(next);
    if (previous && previous.cachePath !== next.cachePath) {
      void removeLocalStl(previous.cachePath).catch(() => undefined);
    }
  }, []);
  const previewSelectedStl = useCallback(async (filePath: string, filename: string) => {
    setStagingLocalFile(true);
    try {
      replaceLocalFile(await stageSelectedStl(filePath, filename));
    } finally {
      setStagingLocalFile(false);
    }
  }, [replaceLocalFile]);
  const replaceFromPicker = useCallback(async () => {
    const selected = await open({
      multiple: false,
      directory: false,
      title: t("Choose an STL file to preview"),
      filters: [{ name: t("STL files"), extensions: ["stl"] }],
    });
    if (!selected || Array.isArray(selected)) return;
    await previewSelectedStl(selected, filenameFromFilePath(selected) || "model.stl");
  }, [previewSelectedStl]);
  const discardLocalFile = useCallback(() => {
    const current = localFileRef.current;
    localFileRef.current = undefined;
    setLocalFile(undefined);
    setResumeLocalSave(false);
    if (current) void removeLocalStl(current.cachePath).catch(() => undefined);
  }, []);
  useEffect(() => {
    let active = true;
    let unlisten: UnlistenFn | undefined;
    const drain = async () => {
      if (drainRunning.current) return;
      drainRunning.current = true;
      setStagingLocalFile(true);
      try {
        while (active) {
          const opened = await takeOpenedStls();
          if (!opened.length) break;
          for (const item of opened) {
            try {
              const staged = await stageOpenedStl(item.id);
              if (active) replaceLocalFile(staged);
              else await removeLocalStl(staged.cachePath).catch(() => undefined);
            } catch (reason) {
              if (active) setIncomingError(localizedError(reason));
            }
          }
        }
      } catch {
        // The event bridge may be unavailable in browser-only previews.
      } finally {
        drainRunning.current = false;
        if (active) setStagingLocalFile(false);
      }
    };
    void listen("volum://opened-stl", () => { void drain(); })
      .then((stop) => {
        if (!active) stop();
        else {
          unlisten = stop;
          void drain();
        }
      })
      .catch(() => { void drain(); });
    return () => {
      active = false;
      unlisten?.();
    };
  }, [replaceLocalFile]);
  const unpair = async () => {
    if (credential && isDemoCredential(credential)) {
      queryClient.clear();
      setCredential(undefined);
      return;
    }
    if (!window.confirm(t("Unpair this device and remove its cached credentials?"))) return;
    let cacheError = "";
    if (credential) {
      const results = await Promise.allSettled([
        clearOfflineProjects(credential.serverId),
        clearMediaCache(credential.serverId),
      ]);
      const failure = results.find((result): result is PromiseRejectedResult => result.status === "rejected");
      if (failure) cacheError = failure.reason instanceof Error ? failure.reason.message : String(failure.reason);
    }
    await clearCredential();
    queryClient.clear();
    setCredential(undefined);
    if (cacheError) window.alert(t("This device was unpaired, but its offline cache could not be fully removed: {error}", { error: localizedError(cacheError) }));
  };
  let content;
  if (stagingLocalFile && !localFile) {
    content = <main className="local-stl-opening"><Brand /><LoaderCircle className="spin" /><strong>{t("Opening STL on this device…")}</strong><span>{t("Nothing is being uploaded.")}</span></main>;
  } else if (localFile && pairingForLocalFile) {
    content = <PairingScreen
      onPaired={(next) => { setCredential(next); setPairingForLocalFile(false); }}
      onDemo={() => undefined}
      onCancel={() => { setPairingForLocalFile(false); setResumeLocalSave(false); }}
    />;
  } else if (localFile) {
    content = <Suspense fallback={<ScreenFallback />}><LocalStlPreviewScreen
      file={localFile}
      credential={credential}
      startSaving={resumeLocalSave}
      onSavingStarted={() => setResumeLocalSave(false)}
      onDiscard={discardLocalFile}
      onReplace={replaceFromPicker}
      onPair={() => { setResumeLocalSave(true); setPairingForLocalFile(true); }}
      onSaved={(projectId) => { discardLocalFile(); setInitialProjectId(projectId); }}
    /></Suspense>;
  } else if (loading) {
    content = <main className="launch-screen"><Brand /><LoaderCircle className="spin" /></main>;
  } else if (unverifiedCredential) {
    content = <main className="launch-screen identity-verification-error"><Brand /><CircleAlert /><strong>{t("Server identity could not be verified")}</strong><span>{verificationError}</span><button className="button button--primary" disabled={loading} onClick={() => void verifySavedCredential(unverifiedCredential)}>{loading ? <LoaderCircle className="spin" /> : <Wifi />}{t("Try again")}</button><button className="text-button" disabled={loading} onClick={() => void forgetUnverifiedCredential()}>{t("Forget saved server")}</button></main>;
  } else {
    content = credential
      ? <CompanionApp credential={credential} initialProjectId={initialProjectId} onInitialProjectOpened={() => setInitialProjectId(undefined)} onCredentialChange={setCredential} onUnpair={() => void unpair()} onPreviewStl={previewSelectedStl} />
      : <PairingScreen onPaired={setCredential} onDemo={() => setCredential(demoCredential)} />;
  }
  return <>{content}{incomingError && <div className="sheet-scrim"><section className="mobile-sheet incoming-file-error" role="alertdialog" aria-modal="true"><header><div><span className="eyebrow">{t("OPEN STL")}</span><h2>{t("This file could not be opened")}</h2></div><button className="round-button" aria-label={t("Close")} onClick={() => setIncomingError("")}><X /></button></header><div className="error-banner"><CircleAlert />{incomingError}</div><button className="button button--primary button--large sheet-submit" onClick={() => setIncomingError("")}>{t("Continue")}</button></section></div>}</>;
}
