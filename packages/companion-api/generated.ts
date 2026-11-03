// Generated from openapi.json by scripts/generate.mjs. Do not edit by hand.

export type ApiError = {
  "code": string;
  "message": string;
  "retryable": boolean;
};

export type ApiErrorEnvelope = {
  "error": ApiError;
};

export type ServerIdentity = {
  "serverId": string;
  "name": string;
  "protocolMin": number;
  "protocolMax": number;
};

export type PairResponse = {
  "serverId": string;
  "deviceId": string;
  "token": string;
  "protocolVersion": number;
};

export type ServerCapabilities = {
  "readLibrary": boolean;
  "organizationWrites": boolean;
  "photoUploads": boolean;
  "fileUploads": boolean;
  "viewerMeshes": boolean;
  "plateThumbnails": boolean;
  "costEstimates": boolean;
  "relatedProjects": boolean;
  "liveEvents": boolean;
  "originalFiles"?: boolean;
  "incrementalSync"?: boolean;
};

export type ServerStatus = {
  "serverId": string;
  "name": string;
  "protocolVersion": number;
  "projectCount": number;
  "libraryRevision": string;
  "capabilities": ServerCapabilities;
};

export type WebBootstrap = {
  "configured": boolean;
  "serverId": string;
  "serverName": string;
  "publicUrl": string;
  "secureTransport": boolean;
  "protocolVersion": number;
};

export type CompanionDevice = {
  "id": string;
  "name": string;
  "platform": string;
  "pairedAt": string;
  "lastSeenAt"?: string;
  "role": "admin" | "client" | "migration";
};

export type PairingInfo = {
  "serverId": string;
  "serverName": string;
  "baseUrl": string;
  "secret": string;
  "expiresAt": string;
  "transport": "tailscale-http" | "private-http" | "https";
  "qrPayload": string;
};
