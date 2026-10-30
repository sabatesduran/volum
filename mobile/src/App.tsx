import { useCallback, useEffect, useRef, useState } from "react";
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
import { LibraryScreen, type Tab } from "./features/library/LibraryScreen";
import { CollectionsScreen } from "./features/organization/CollectionsScreen";
import { ProjectDetailScreen } from "./features/project/ProjectDetailScreen";
import { SettingsScreen } from "./features/settings/SettingsScreen";
import { LocalStlPreviewScreen } from "./features/upload/LocalStlPreviewScreen";
import { UploadScreen } from "./features/upload/UploadScreen";
import { clearCredential, loadCredential, saveCredential } from "./lib/credentials";
import { clearOfflineProjects } from "./lib/offline";
import { clearMediaCache } from "./lib/mediaCache";
import { localizedError, t, useLanguage } from "./lib/i18n";
import {
  removeLocalStl,
  stageOpenedStl,
  stageSelectedStl,
  takeOpenedStls,
  type LocalStlFile,
} from "./lib/localStl";
import { filenameFromFilePath, pairWithDesktop, parsePairingPayload, requestJson } from "./lib/transport";
import type { Credential, ServerStatus } from "./types";

export function PairingScreen({ onPaired, onCancel }: { onPaired: (credential: Credential) => void; onCancel?: () => void }) {
  const [manual, setManual] = useState(false);
  const [payloadText, setPayloadText] = useState("");
  const [busy, setBusy] = useState(false);
  const [scanning, setScanning] = useState(false);
  const [error, setError] = useState("");
  const scanCanceled = useRef(false);

  const connect = async (value: string) => {
    setBusy(true); setError("");
    try {
      const payload = parsePairingPayload(value);
      const credential = await pairWithDesktop(payload);
      await saveCredential(credential);
      onPaired(credential);
    } catch (reason) {
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
      await connect(result.content);
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
    <header><Brand /><span className="companion-label">{t("MOBILE")}</span></header>
    <section className="pairing-intro">
      <div className="pairing-icon-stage" aria-hidden="true"><span /><span /><img src={volumIcon} alt="" /></div>
      <div className="eyebrow">{t("Your private 3D library")}</div>
      <h1>{t("From desktop")}<br />{t("to workbench.")}</h1>
      <p>{t("Keep project files, photos, parts, and build steps close while you print.")}</p>
    </section>
    <section className="pairing-card">
      <div className="pairing-card__heading"><span><QrCode /></span><div><strong>{t("Connect to Volum Desktop")}</strong><p>{t("Open Settings → Companion Access on your Mac or PC.")}</p></div></div>
      {manual ? <div className="manual-pairing">
        <label><span>{t("Pairing data")}</span><textarea value={payloadText} onChange={(event) => setPayloadText(event.target.value)} placeholder={t("Paste from Volum Desktop")} autoCapitalize="off" autoCorrect="off" /></label>
        <button className="button button--primary button--large" disabled={busy || !payloadText.trim()} onClick={() => void connect(payloadText)}>{busy ? <LoaderCircle className="spin" /> : <Wifi />}{t("Connect securely")}</button>
        <button className="text-button" onClick={() => setManual(false)}>{t("Use the camera instead")}</button>
      </div> : <div className="pairing-actions">
        <button className="button button--primary button--large" disabled={busy || scanning} onClick={() => void scanCode()}>{busy || scanning ? <LoaderCircle className="spin" /> : <QrCode />}{t("Scan desktop code")}</button>
        <button className="text-button" disabled={scanning} onClick={() => setManual(true)}>{t("Enter pairing data manually")}</button>
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
  if (projectId) return <ProjectDetailScreen key={projectId} credential={credential} id={projectId} canWrite={canWrite} onBack={() => setProjectId(undefined)} onOpen={setProjectId} />;
  const chooseTab = (next: Tab) => { setTab(next); if (next !== "library") setCollectionId(undefined); };
  return <div className="app-shell">
    {tab === "collections" ? <CollectionsScreen credential={credential} canWrite={canWrite} onSelect={(id) => { setCollectionId(id); setTab("library"); }} onOpen={setProjectId} /> : tab === "upload" ? <UploadScreen credential={credential} photosEnabled={Boolean(status.data?.capabilities.photoUploads)} onOpen={setProjectId} onPreviewStl={onPreviewStl} /> : tab === "settings" ? <SettingsScreen credential={credential} status={status.data} onCredentialChange={onCredentialChange} onRefresh={() => status.refetch()} onUnpair={onUnpair} /> : <LibraryScreen credential={credential} tab={tab} selectedCollection={collectionId} onCollection={setCollectionId} onOpen={setProjectId} />}
    <nav className="bottom-nav">{([
      ["library", "Library", LibraryBig], ["favorites", "Favorites", Heart], ["upload", "Capture", Camera], ["collections", "Collections", Boxes], ["settings", "Settings", Settings2]
    ] as Array<[Tab, string, typeof LibraryBig]>).map(([id, label, Icon]) => <button key={id} className={tab === id ? "is-active" : ""} onClick={() => chooseTab(id)}><Icon size={20} fill={id === "favorites" && tab === id ? "currentColor" : "none"} /><span>{t(label)}</span></button>)}</nav>
  </div>;
}

export function App() {
  useLanguage();
  const queryClient = useQueryClient();
  const [credential, setCredential] = useState<Credential>();
  const [loading, setLoading] = useState(true);
  const [startupError, setStartupError] = useState("");
  const [localFile, setLocalFile] = useState<LocalStlFile>();
  const localFileRef = useRef<LocalStlFile | undefined>(undefined);
  const [stagingLocalFile, setStagingLocalFile] = useState(false);
  const [incomingError, setIncomingError] = useState("");
  const [pairingForLocalFile, setPairingForLocalFile] = useState(false);
  const [resumeLocalSave, setResumeLocalSave] = useState(false);
  const [initialProjectId, setInitialProjectId] = useState<string>();
  const drainRunning = useRef(false);
  useEffect(() => {
    void loadCredential().then(setCredential).catch((reason) => setStartupError(localizedError(reason))).finally(() => setLoading(false));
  }, []);
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
      onCancel={() => { setPairingForLocalFile(false); setResumeLocalSave(false); }}
    />;
  } else if (localFile) {
    content = <LocalStlPreviewScreen
      file={localFile}
      credential={credential}
      startSaving={resumeLocalSave}
      onSavingStarted={() => setResumeLocalSave(false)}
      onDiscard={discardLocalFile}
      onReplace={replaceFromPicker}
      onPair={() => { setResumeLocalSave(true); setPairingForLocalFile(true); }}
      onSaved={(projectId) => { discardLocalFile(); setInitialProjectId(projectId); }}
    />;
  } else if (loading) {
    content = <main className="launch-screen"><Brand /><LoaderCircle className="spin" /></main>;
  } else if (startupError) {
    content = <main className="launch-screen"><Brand /><div className="error-banner" role="alert"><CircleAlert />{startupError}</div></main>;
  } else {
    content = credential
      ? <CompanionApp credential={credential} initialProjectId={initialProjectId} onInitialProjectOpened={() => setInitialProjectId(undefined)} onCredentialChange={setCredential} onUnpair={() => void unpair()} onPreviewStl={previewSelectedStl} />
      : <PairingScreen onPaired={setCredential} />;
  }
  return <>{content}{incomingError && <div className="sheet-scrim"><section className="mobile-sheet incoming-file-error" role="alertdialog" aria-modal="true"><header><div><span className="eyebrow">{t("OPEN STL")}</span><h2>{t("This file could not be opened")}</h2></div><button className="round-button" aria-label={t("Close")} onClick={() => setIncomingError("")}><X /></button></header><div className="error-banner"><CircleAlert />{incomingError}</div><button className="button button--primary button--large sheet-submit" onClick={() => setIncomingError("")}>{t("Continue")}</button></section></div>}</>;
}
