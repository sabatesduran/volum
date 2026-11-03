import type { WebCredential } from "./credential";
import { CompanionApiError, CompanionClient, createFetchTransport } from "@volum/companion-api/client";
import type {
  CompanionDevice,
  PairingInfo,
  WebBootstrap,
} from "@volum/companion-api/generated";

export type Bootstrap = WebBootstrap;
export type Device = CompanionDevice;
export type Pairing = PairingInfo;
export { CompanionApiError as WebApiError };

const publicClient = new CompanionClient(createFetchTransport());

export function bootstrap() {
  return publicClient.json<Bootstrap>("/api/v1/web/bootstrap");
}

export function login(password: string, deviceName: string) {
  return publicClient.json<WebCredential & { protocolVersion: number }>(
    "/api/v1/web/login",
    "POST",
    { password, deviceName },
  );
}

function authorized<T>(credential: WebCredential, path: string, method = "GET") {
  return new CompanionClient(createFetchTransport(), credential.token)
    .json<T>(path, method as "GET" | "POST" | "DELETE");
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
