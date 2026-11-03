import { mockIPC } from "@tauri-apps/api/mocks";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createRoot } from "react-dom/client";
import { useState } from "react";
import { CompanionApp, PairingScreen } from "./App";
import { LocalStlPreviewScreen } from "./features/upload/LocalStlPreviewScreen";
import { demoCredential } from "./lib/demo";
import { resolveLanguage, setLanguage } from "./lib/i18n";
import { visualCredential, visualJsonResponse, visualLocalStlEnvelope, visualMediaEnvelope } from "./visualFixtures";
import "@volum/design/tokens.css";
import "@volum/design/primitives.css";
import "./styles.css";

const parameters = new URLSearchParams(window.location.search);
const language = parameters.get("language");
const connectionOffline = parameters.get("connection") === "offline";
const mediaMissing = parameters.get("media") === "missing";
const mediaFailsAfterFirstLoad = parameters.get("media") === "fail-after-first";
const mediaRequests = new Map<string, number>();
if (language) setLanguage(resolveLanguage(language));

mockIPC((command, payload) => {
  if (command === "plugin:app|version") return "1.0.0";
  if (command === "plugin:barcode-scanner|request_permissions") return { camera: "granted" };
  if (command === "plugin:barcode-scanner|scan") return new Promise(() => undefined);
  if (command === "plugin:barcode-scanner|cancel") return undefined;
  if (command === "companion_json_request") {
    if (connectionOffline) throw new Error("Could not reach the Volum server through the selected network.");
    const request = (payload as { request?: { path?: string } } | undefined)?.request;
    const data = visualJsonResponse(request?.path ?? "");
    return data === undefined
      ? { status: 404, contentType: "application/json", body: JSON.stringify({ error: { code: "not_found", message: "Visual fixture not found." } }) }
      : { status: 200, contentType: "application/json", body: JSON.stringify(data) };
  }
  if (command === "companion_media_request") {
    if (connectionOffline) throw new Error("Could not reach the Volum server through the selected network.");
    if (mediaMissing) throw new Error("Volum Desktop returned 404");
    const request = (payload as { request?: { path?: string } } | undefined)?.request;
    const path = request?.path ?? "media";
    const count = mediaRequests.get(path) ?? 0;
    mediaRequests.set(path, count + 1);
    if (mediaFailsAfterFirstLoad && count > 0) throw new Error("This media item is not available");
    return visualMediaEnvelope(path);
  }
  if (command === "companion_preview_local_stl") return visualLocalStlEnvelope();
  if (command === "companion_take_opened_stls") return [];
  throw new Error(`Unexpected visual fixture command: ${command}`);
});

const queryClient = new QueryClient({
  defaultOptions: { queries: { retry: false, staleTime: Infinity } },
});
const screen = parameters.get("screen") ?? "library";

function VisualApp() {
  const [demo, setDemo] = useState(false);
  if (screen === "local-stl") {
    return <LocalStlPreviewScreen
      file={{ cachePath: "/visual/fixture.stl", filename: "workbench-bracket.stl", byteSize: 1_842_000 }}
      credential={visualCredential}
      onSavingStarted={() => undefined}
      onDiscard={() => undefined}
      onReplace={async () => undefined}
      onPair={() => undefined}
      onSaved={() => undefined}
    />;
  }
  if (screen === "pairing" && !demo) {
    return <PairingScreen onPaired={() => undefined} onDemo={() => setDemo(true)} />;
  }
  const credential = demo ? demoCredential : visualCredential;
  return <CompanionApp credential={credential} onCredentialChange={() => undefined} onUnpair={() => setDemo(false)} onPreviewStl={async () => undefined} verified onVerified={() => undefined} onRevoked={() => undefined} />;
}

createRoot(document.getElementById("root")!).render(
  <QueryClientProvider client={queryClient}>
    <VisualApp />
  </QueryClientProvider>,
);
