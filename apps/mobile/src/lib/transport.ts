import { Channel, invoke } from "@tauri-apps/api/core";
import { appCacheDir, join } from "@tauri-apps/api/path";
import { remove, writeFile } from "@tauri-apps/plugin-fs";
import {
  CompanionApiError,
  CompanionClient,
  type CompanionMethod,
  type CompanionTransport,
} from "@volum/companion-api/client";
import type { Credential, PairingPayload, ProjectDocumentation, UploadProgress, UploadResult } from "../types";
import { demoJsonResponse, demoMediaEnvelope, isDemoCredential } from "./demo";

interface NativeResponse {
  status: number;
  contentType?: string;
  body: string;
}

export const CompanionError = CompanionApiError;

const CONNECTION_ERROR_MESSAGES = new Set([
  "Could not reach the Volum server through the selected network.",
  "Volum Desktop did not respond. Check that it is open and Companion Access is on.",
  "The connection to Volum Desktop failed.",
]);
const verifiedOrigins = new Set<string>();
const UNVERIFIED_ORIGIN_ERROR = "Volum Desktop is unreachable. Showing offline copies.";

export function markOriginVerified(value: string) {
  verifiedOrigins.add(normalizeServerUrl(value));
}

export function isOriginVerified(value: string) {
  try {
    return verifiedOrigins.has(normalizeServerUrl(value));
  } catch {
    return false;
  }
}

export function isConnectionError(reason: unknown) {
  if (reason instanceof CompanionApiError && ["connection_failed", "server_unverified"].includes(reason.code ?? "")) return true;
  const message = reason instanceof Error ? reason.message : String(reason);
  return CONNECTION_ERROR_MESSAGES.has(message);
}

export function retryCompanionQuery(failureCount: number, error: unknown) {
  return !isConnectionError(error) && failureCount < 1;
}

type JsonMethod = CompanionMethod;

function nativeTransport(baseUrl: string): CompanionTransport {
  return {
    async request(request) {
      const response = await invoke<NativeResponse>("companion_json_request", {
        request: {
          baseUrl,
          path: request.path,
          token: request.token,
          body: request.body,
          method: request.method,
        },
      });
      return { status: response.status, body: response.body, contentType: response.contentType };
    },
  };
}

async function nativeJson<T>(baseUrl: string, path: string, token?: string, body?: unknown, method?: JsonMethod): Promise<T> {
  return new CompanionClient(nativeTransport(baseUrl), token).json<T>(path, method, body);
}

export async function pairWithDesktop(payload: PairingPayload): Promise<Credential> {
  const baseUrl = normalizeServerUrl(payload.baseUrl);
  const identity = await nativeJson<{ serverId: string; protocolMin: number; protocolMax: number }>(baseUrl, "/identity");
  if (identity.serverId !== payload.serverId || identity.protocolMin > 1 || identity.protocolMax < 1) {
    verifiedOrigins.delete(baseUrl);
    throw new CompanionApiError("This address belongs to a different or incompatible Volum server.", 0, "server_identity_mismatch");
  }
  const platform = /android/i.test(navigator.userAgent) ? "Android" : /iphone|ipad/i.test(navigator.userAgent) ? "iOS" : "Companion";
  const result = await nativeJson<{ serverId: string; deviceId: string; token: string; protocolVersion: number }>(
    baseUrl,
    "/pair",
    undefined,
    { secret: payload.secret, deviceName: deviceName(), platform },
    "POST",
  );
  if (result.serverId !== payload.serverId || result.protocolVersion !== 1) {
    verifiedOrigins.delete(baseUrl);
    throw new CompanionApiError("This Volum Desktop identity changed during pairing.", 0, "server_identity_mismatch");
  }
  markOriginVerified(baseUrl);
  return { serverId: result.serverId, serverName: payload.serverName, baseUrl, deviceId: result.deviceId, token: result.token };
}

export async function verifyServerIdentity(baseUrl: string, expectedServerId: string) {
  const normalized = normalizeServerUrl(baseUrl);
  const identity = await nativeJson<{ serverId: string; protocolMin: number; protocolMax: number }>(normalized, "/identity");
  if (identity.serverId !== expectedServerId || identity.protocolMin > 1 || identity.protocolMax < 1) {
    verifiedOrigins.delete(normalized);
    throw new CompanionApiError("That address belongs to a different Volum server.", 0, "server_identity_mismatch");
  }
  markOriginVerified(normalized);
  return normalized;
}

export async function requestJson<T>(credential: Credential, path: string): Promise<T> {
  if (isDemoCredential(credential)) {
    const response = demoJsonResponse(path);
    if (response === undefined) throw new CompanionApiError("This item is not available in the demo library.", 404, "not_found");
    return response as T;
  }
  requireVerifiedOrigin(credential);
  return nativeJson<T>(credential.baseUrl, path, credential.token, undefined, "GET");
}

export function mutateJson<T>(credential: Credential, path: string, method: Exclude<JsonMethod, "GET">, body?: unknown): Promise<T> {
  if (isDemoCredential(credential)) return Promise.reject(new CompanionApiError("The demo library is read-only.", 403, "demo_read_only"));
  try {
    requireVerifiedOrigin(credential);
  } catch (reason) {
    return Promise.reject(reason);
  }
  return nativeJson<T>(credential.baseUrl, path, credential.token, body, method);
}

export async function requestMedia(credential: Credential, path: string, mimeType = "image/png"): Promise<string> {
  return URL.createObjectURL(await requestBlob(credential, path, mimeType));
}

export async function requestBlob(credential: Credential, path: string, mimeType = "application/octet-stream"): Promise<Blob> {
  const bytes = await requestBinary(credential, path);
  return new Blob([bytes], { type: mimeType });
}

export function requestBinary(credential: Credential, path: string): Promise<ArrayBuffer> {
  return requestMediaResponse(credential, path).then((response) => {
    if (response.status !== 200) throw new CompanionError("Volum Desktop returned an invalid media response.");
    return response.bytes;
  });
}

export interface MediaResponse {
  status: 200 | 304;
  etag?: string;
  contentType?: string;
  bytes: ArrayBuffer;
}

export async function requestMediaResponse(
  credential: Credential,
  path: string,
  ifNoneMatch?: string,
): Promise<MediaResponse> {
  if (isDemoCredential(credential)) return parseMediaEnvelope(demoMediaEnvelope(path));
  requireVerifiedOrigin(credential);
  const envelope = await invoke<ArrayBuffer>("companion_media_request", {
    request: {
      baseUrl: credential.baseUrl,
      path,
      token: credential.token,
      ifNoneMatch,
    },
  });
  return parseMediaEnvelope(envelope);
}

export function parseMediaEnvelope(envelope: ArrayBuffer): MediaResponse {
  const bytes = new Uint8Array(envelope);
  if (bytes.byteLength < 10 || String.fromCharCode(...bytes.subarray(0, 4)) !== "VMC1") {
    throw new CompanionError("Volum Desktop returned an unreadable media response.");
  }
  const view = new DataView(envelope);
  const status = view.getUint16(4, true);
  const etagLength = view.getUint16(6, true);
  const contentTypeLength = view.getUint16(8, true);
  const bodyOffset = 10 + etagLength + contentTypeLength;
  if (
    (status !== 200 && status !== 304)
    || bodyOffset > bytes.byteLength
    || (status === 304 && bodyOffset !== bytes.byteLength)
  ) {
    throw new CompanionError("Volum Desktop returned an unreadable media response.");
  }
  const decoder = new TextDecoder();
  const etag = decoder.decode(bytes.subarray(10, 10 + etagLength)) || undefined;
  const contentType = decoder.decode(bytes.subarray(10 + etagLength, bodyOffset)) || undefined;
  return {
    status,
    etag,
    contentType,
    bytes: envelope.slice(bodyOffset),
  };
}

export async function uploadProjectFile(
  credential: Credential,
  filePath: string,
  filename: string,
  destination: { projectId: string } | { projectName: string },
  onProgress: (progress: UploadProgress) => void,
  uploadId: string,
): Promise<UploadResult> {
  if (isDemoCredential(credential)) throw new CompanionError("The demo library is read-only.", 403, "demo_read_only");
  requireVerifiedOrigin(credential);
  const progress = new Channel<UploadProgress>();
  progress.onmessage = onProgress;
  return invoke<UploadResult>("companion_upload_file", {
    request: {
      baseUrl: credential.baseUrl,
      token: credential.token,
      filePath,
      filename,
      uploadId,
      ...destination
    },
    progress
  });
}

export async function uploadProjectPhoto(
  credential: Credential,
  file: File,
  projectId: string,
  caption: string,
  ifUnmodifiedSince: string,
  onProgress: (progress: UploadProgress) => void,
  uploadId: string,
): Promise<ProjectDocumentation> {
  if (isDemoCredential(credential)) throw new CompanionError("The demo library is read-only.", 403, "demo_read_only");
  requireVerifiedOrigin(credential);
  if (file.size <= 0 || file.size > 25 * 1024 * 1024) throw new CompanionError("Project photos must be non-empty and at most 25 MB.");
  const extension = photoExtension(file);
  const originalBase = file.name.replace(/\.[^.]+$/, "").trim().slice(0, 140) || "workshop-photo";
  const filename = `${originalBase}.${extension}`;
  const temporaryPath = await join(await appCacheDir(), `volum-photo-${crypto.randomUUID()}.${extension}`);
  await writeFile(temporaryPath, new Uint8Array(await file.arrayBuffer()));
  const progress = new Channel<UploadProgress>();
  progress.onmessage = onProgress;
  try {
    return await invoke<ProjectDocumentation>("companion_upload_photo", {
      request: {
        baseUrl: credential.baseUrl,
        token: credential.token,
        filePath: temporaryPath,
        filename,
        projectId,
        caption: caption.trim(),
        ifUnmodifiedSince,
        uploadId,
      },
      progress
    });
  } finally {
    await remove(temporaryPath).catch(() => undefined);
  }
}

export function cancelUpload(uploadId: string): Promise<boolean> {
  return invoke<boolean>("companion_cancel_upload", { uploadId });
}

export function normalizeServerUrl(value: string): string {
  const input = value.trim();
  if (!input) throw new CompanionError("Enter the Volum server address.");
  let url: URL;
  const hasScheme = /^https?:\/\//i.test(input);
  const rawIpv6 = !hasScheme && !input.startsWith("[") && (input.match(/:/g)?.length ?? 0) > 1;
  const bracketedIpv6 = !hasScheme && /^\[[0-9a-f:]+\](?::\d+)?$/i.test(input);
  const privateIpv4 = /^(10\.|192\.168\.|172\.(1[6-9]|2\d|3[01])\.|100\.(6[4-9]|[7-9]\d|1[01]\d|12[0-7])\.)/.test(input);
  const localName = /(?:\.local|\.ts\.net)$/i.test(input);
  try { url = new URL(hasScheme ? input : rawIpv6 ? `http://[${input}]` : bracketedIpv6 || privateIpv4 || localName ? `http://${input}` : `https://${input}`); }
  catch { throw new CompanionError("That server address is not valid."); }
  if (url.username || url.password || url.search || url.hash || (url.pathname && url.pathname !== "/")) {
    throw new CompanionError("Enter only the Volum server origin, without a path, query, or credentials.");
  }
  if (!url.port && url.protocol === "http:") url.port = "43821";
  return url.origin;
}

function requireVerifiedOrigin(credential: Credential) {
  if (!isOriginVerified(credential.baseUrl)) {
    throw new CompanionApiError(UNVERIFIED_ORIGIN_ERROR, 0, "server_unverified", true);
  }
}

export function transportForServerUrl(value: string, original?: PairingPayload): PairingPayload["transport"] {
  const baseUrl = normalizeServerUrl(value);
  if (baseUrl.startsWith("https://")) return "https";
  if (original?.transport === "tailscale-http" && baseUrl === normalizeServerUrl(original.baseUrl)) return "tailscale-http";
  return "private-http";
}

export function pairingPayloadWithAddress(payload: PairingPayload, value: string): PairingPayload {
  const baseUrl = normalizeServerUrl(value);
  return { ...payload, baseUrl, transport: transportForServerUrl(baseUrl, payload) };
}

export function filenameFromFilePath(value: string): string {
  const path = value.split(/[?#]/, 1)[0];
  const last = path.split(/[\\/]/).pop() ?? "";
  let decoded = last;
  try { decoded = decodeURIComponent(last); } catch { /* keep the original segment */ }
  if (decoded.includes(":")) decoded = decoded.slice(decoded.lastIndexOf(":") + 1);
  return decoded.split(/[\\/]/).pop() ?? "";
}

export function parsePairingPayload(value: string): PairingPayload {
  let payload: Partial<PairingPayload>;
  try { payload = JSON.parse(value) as Partial<PairingPayload>; }
  catch { throw new CompanionError("That is not a Volum pairing code."); }
  if (payload.version !== 1 || !["tailscale-http", "private-http", "https"].includes(String(payload.transport)) || !payload.serverId || !payload.serverName || !payload.baseUrl || !payload.secret || !payload.expiresAt) {
    throw new CompanionError("That is not a compatible Volum pairing code.");
  }
  if (Date.parse(payload.expiresAt) <= Date.now()) throw new CompanionError("This pairing code has expired. Create a new one on Volum Desktop.");
  return payload as PairingPayload;
}

function deviceName(): string {
  if (/ipad/i.test(navigator.userAgent)) return "iPad";
  if (/iphone/i.test(navigator.userAgent)) return "iPhone";
  if (/android/i.test(navigator.userAgent)) return "Android phone";
  return "Volum mobile";
}

function photoExtension(file: File) {
  if (file.type === "image/png") return "png";
  if (file.type === "image/webp") return "webp";
  if (file.type === "image/jpeg") return "jpg";
  const extension = file.name.split(".").pop()?.toLowerCase();
  if (extension === "jpeg") return "jpg";
  if (extension && ["jpg", "png", "webp"].includes(extension)) return extension;
  throw new CompanionError("Choose a JPEG, PNG, or WebP photo.");
}
