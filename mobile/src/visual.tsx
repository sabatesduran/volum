import { mockIPC } from "@tauri-apps/api/mocks";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createRoot } from "react-dom/client";
import { CompanionApp, PairingScreen } from "./App";
import { setLanguage, type AppLanguage } from "./lib/i18n";
import { visualCredential, visualJsonResponse, visualMediaEnvelope } from "./visualFixtures";
import "./styles.css";

const parameters = new URLSearchParams(window.location.search);
const language = parameters.get("language");
const connectionOffline = parameters.get("connection") === "offline";
if (language === "ca" || language === "es" || language === "en") setLanguage(language as AppLanguage);

mockIPC((command, payload) => {
  if (command === "plugin:barcode-scanner|request_permissions") return { camera: "granted" };
  if (command === "plugin:barcode-scanner|scan") return new Promise(() => undefined);
  if (command === "plugin:barcode-scanner|cancel") return undefined;
  if (command === "companion_json_request") {
    if (connectionOffline) throw new Error("Could not reach Volum Desktop through Tailscale.");
    const request = (payload as { request?: { path?: string } } | undefined)?.request;
    const data = visualJsonResponse(request?.path ?? "");
    return data === undefined
      ? { status: 404, contentType: "application/json", body: JSON.stringify({ error: { code: "not_found", message: "Visual fixture not found." } }) }
      : { status: 200, contentType: "application/json", body: JSON.stringify(data) };
  }
  if (command === "companion_media_request") {
    if (connectionOffline) throw new Error("Could not reach Volum Desktop through Tailscale.");
    const request = (payload as { request?: { path?: string } } | undefined)?.request;
    return visualMediaEnvelope(request?.path ?? "media");
  }
  throw new Error(`Unexpected visual fixture command: ${command}`);
});

const queryClient = new QueryClient({
  defaultOptions: { queries: { retry: false, staleTime: Infinity } },
});
const screen = parameters.get("screen") ?? "library";

createRoot(document.getElementById("root")!).render(
  <QueryClientProvider client={queryClient}>
    {screen === "pairing"
      ? <PairingScreen onPaired={() => undefined} />
      : <CompanionApp credential={visualCredential} onCredentialChange={() => undefined} onUnpair={() => undefined} onPreviewStl={async () => undefined} />}
  </QueryClientProvider>,
);
