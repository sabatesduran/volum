import type { WebCredential } from "./credential";

export interface Bootstrap {
  configured: boolean;
  serverId: string;
  serverName: string;
  publicUrl: string;
  secureTransport: boolean;
  protocolVersion: number;
}

export interface Device {
  id: string;
  name: string;
  platform: string;
  pairedAt: string;
  lastSeenAt?: string;
  role: "admin" | "client" | "migration";
}

export interface Pairing {
  serverId: string;
  serverName: string;
  baseUrl: string;
  secret: string;
  expiresAt: string;
  transport: "tailscale-http" | "private-http" | "https";
  qrPayload: string;
}

interface ErrorEnvelope {
  error?: { code?: string; message?: string };
}

export class WebApiError extends Error {
  constructor(message: string, readonly status: number, readonly code = "request_failed") {
    super(message);
  }
}

async function responseJson<T>(response: Response): Promise<T> {
  if (!response.ok) {
    let envelope: ErrorEnvelope = {};
    try { envelope = await response.json() as ErrorEnvelope; } catch { /* normalized below */ }
    throw new WebApiError(envelope.error?.message ?? `Volum returned ${response.status}`, response.status, envelope.error?.code);
  }
  if (response.status === 204) return undefined as T;
  return response.json() as Promise<T>;
}

export function bootstrap() {
  return fetch("/api/v1/web/bootstrap", { headers: { Accept: "application/json" } }).then(responseJson<Bootstrap>);
}

export function login(password: string, deviceName: string) {
  return fetch("/api/v1/web/login", {
    method: "POST",
    headers: { "Content-Type": "application/json", Accept: "application/json" },
    body: JSON.stringify({ password, deviceName }),
  }).then(responseJson<WebCredential & { protocolVersion: number }>);
}

function authorized<T>(credential: WebCredential, path: string, method = "GET") {
  return fetch(path, {
    method,
    headers: { Authorization: `Bearer ${credential.token}`, Accept: "application/json" },
  }).then(responseJson<T>);
}

export function listDevices(credential: WebCredential) {
  return authorized<Device[]>(credential, "/api/v1/admin/devices");
}

export function createPairing(credential: WebCredential) {
  return authorized<Pairing>(credential, "/api/v1/admin/pairings", "POST");
}

export function createMigrationPairing(credential: WebCredential) {
  return authorized<Pairing>(credential, "/api/v1/admin/migration-pairings", "POST");
}

export function revokeDevice(credential: WebCredential, id: string) {
  return authorized<void>(credential, `/api/v1/admin/devices/${encodeURIComponent(id)}`, "DELETE");
}

export function logout(credential: WebCredential) {
  return authorized<void>(credential, "/api/v1/web/logout", "POST");
}
