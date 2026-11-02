import type { Credential, ServerStatus } from "../types";
import {
  visualJsonResponse,
  visualMediaEnvelope,
  visualStatus,
} from "../visualFixtures";

const DEMO_SERVER_ID = "volum-demo";

export const demoCredential: Credential = {
  serverId: DEMO_SERVER_ID,
  serverName: "Volum Demo",
  baseUrl: "demo://volum",
  deviceId: "demo-device",
  token: "demo-only",
};

const demoStatus: ServerStatus = {
  ...visualStatus,
  serverId: DEMO_SERVER_ID,
  name: demoCredential.serverName,
  capabilities: {
    ...visualStatus.capabilities,
    organizationWrites: false,
    photoUploads: false,
    fileUploads: false,
  },
};

export function isDemoCredential(credential: Credential) {
  return credential.serverId === DEMO_SERVER_ID;
}

export function demoJsonResponse(path: string) {
  return path.split("?", 1)[0] === "/api/v1/status"
    ? demoStatus
    : visualJsonResponse(path);
}

export const demoMediaEnvelope = visualMediaEnvelope;
