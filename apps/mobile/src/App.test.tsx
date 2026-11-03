// @vitest-environment jsdom

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";

const mocks = vi.hoisted(() => ({
  cancel: vi.fn(),
  clearCredential: vi.fn(),
  clearMediaCache: vi.fn(),
  clearOfflineProjects: vi.fn(),
  loadCredential: vi.fn(),
  listOfflineManifests: vi.fn(),
  listOfflineProjectSnapshots: vi.fn(),
  pairWithDesktop: vi.fn(),
  prepareCredentialStore: vi.fn(),
  requestJson: vi.fn(),
  requestPermissions: vi.fn(),
  saveCredential: vi.fn(),
  scan: vi.fn(),
  verifyServerIdentity: vi.fn(),
}));

vi.mock("@tauri-apps/plugin-barcode-scanner", () => ({
  cancel: mocks.cancel,
  Format: { QRCode: "QRCode" },
  requestPermissions: mocks.requestPermissions,
  scan: mocks.scan,
}));

vi.mock("./lib/credentials", () => ({
  clearCredential: mocks.clearCredential,
  loadCredential: mocks.loadCredential,
  prepareCredentialStore: mocks.prepareCredentialStore,
  saveCredential: mocks.saveCredential,
}));

vi.mock("./lib/offline", () => ({
  clearOfflineProjects: mocks.clearOfflineProjects,
  listOfflineManifests: mocks.listOfflineManifests,
  listOfflineProjectSnapshots: mocks.listOfflineProjectSnapshots,
}));

vi.mock("./lib/mediaCache", () => ({
  clearMediaCache: mocks.clearMediaCache,
}));

vi.mock("./lib/transport", async (importOriginal) => ({
  ...await importOriginal<typeof import("./lib/transport")>(),
  pairWithDesktop: mocks.pairWithDesktop,
  requestJson: mocks.requestJson,
  verifyServerIdentity: mocks.verifyServerIdentity,
}));

vi.mock("./features/project/ProjectMedia", () => ({
  ProjectImage: () => null,
}));

import { App, PairingScreen } from "./App";
import { CompanionError } from "./lib/transport";
import type { Credential, PairingPayload } from "./types";

const credential: Credential = {
  serverId: "server-1",
  serverName: "Workshop",
  baseUrl: "http://100.81.102.15:43821",
  deviceId: "device-1",
  token: "device-token",
};

const manifest = {
  schemaVersion: 2,
  key: "server-1:project-1",
  serverId: "server-1",
  projectId: "project-1",
  displayName: "Cached project",
  revision: "revision-1",
  syncedAt: "2026-01-01T00:00:00.000Z",
  byteSize: 128,
  mediaCount: 0,
};

const offlineSnapshot = {
  manifest,
  detail: {
    id: "project-1",
    displayName: "Cached project",
    folderId: "folder-1",
    folderName: "Workshop",
    primaryExtension: "stl",
    favorite: false,
    addedAt: "2026-01-01T00:00:00.000Z",
    modifiedAt: "2026-01-01T00:00:00.000Z",
    missing: false,
    assetCount: 1,
    revision: "revision-1",
    notes: "",
    rootName: "Library",
    assets: [],
    collectionIds: [],
    tags: [],
  },
  documentation: { revision: "revision-1", photos: [], customValues: [], bomItems: [], assemblySteps: [] },
  photoThumbnails: [],
};

function pairingPayload(transport: PairingPayload["transport"] = "tailscale-http"): PairingPayload {
  return {
    version: 1,
    serverId: "server-1",
    serverName: "Workshop",
    baseUrl: transport === "https" ? "https://volum.example.com" : transport === "private-http" ? "http://192.168.1.20:43821" : "http://100.81.102.15:43821",
    secret: "one-use-secret",
    expiresAt: "2999-01-01T00:00:00Z",
    transport,
  };
}

beforeEach(() => {
  mocks.cancel.mockReset().mockResolvedValue(undefined);
  mocks.clearCredential.mockReset().mockResolvedValue(undefined);
  mocks.clearMediaCache.mockReset().mockResolvedValue(undefined);
  mocks.clearOfflineProjects.mockReset().mockResolvedValue(undefined);
  mocks.loadCredential.mockReset().mockResolvedValue(undefined);
  mocks.listOfflineManifests.mockReset().mockResolvedValue([]);
  mocks.listOfflineProjectSnapshots.mockReset().mockResolvedValue([]);
  mocks.pairWithDesktop.mockReset().mockResolvedValue(credential);
  mocks.prepareCredentialStore.mockReset().mockResolvedValue(undefined);
  mocks.requestJson.mockReset().mockRejectedValue(new Error("Unexpected Companion request"));
  mocks.requestPermissions.mockReset().mockResolvedValue("granted");
  mocks.saveCredential.mockReset().mockResolvedValue(undefined);
  mocks.scan.mockReset().mockResolvedValue({ content: JSON.stringify(pairingPayload()) });
  mocks.verifyServerIdentity.mockReset().mockResolvedValue(credential.baseUrl);
});

afterEach(cleanup);

describe("mobile pairing", () => {
  it("connects a scanned Tailscale code without showing the review step", async () => {
    const onPaired = vi.fn();
    render(<PairingScreen onPaired={onPaired} onDemo={vi.fn()} />);

    fireEvent.click(screen.getByRole("button", { name: "Scan desktop code" }));

    await waitFor(() => expect(onPaired).toHaveBeenCalledWith(credential));
    expect(mocks.prepareCredentialStore).toHaveBeenCalledOnce();
    expect(mocks.pairWithDesktop).toHaveBeenCalledWith(pairingPayload());
    expect(mocks.prepareCredentialStore.mock.invocationCallOrder[0]).toBeLessThan(mocks.pairWithDesktop.mock.invocationCallOrder[0]);
    expect(screen.queryByRole("button", { name: "Verify and connect" })).toBeNull();
  });

  it("shows address review only after the automatic connection fails", async () => {
    mocks.pairWithDesktop.mockRejectedValueOnce(new Error("Could not reach the Volum server through the selected network."));
    render(<PairingScreen onPaired={vi.fn()} onDemo={vi.fn()} />);

    fireEvent.click(screen.getByRole("button", { name: "Scan desktop code" }));

    expect(await screen.findByRole("button", { name: "Verify and connect" })).toBeTruthy();
    expect((screen.getByLabelText("Server address") as HTMLInputElement).value).toBe("http://100.81.102.15:43821");
    expect(screen.getByRole("alert").textContent).toContain("Could not reach the Volum server through the selected network.");
  });

  it("keeps manual pairing on the review step", () => {
    render(<PairingScreen onPaired={vi.fn()} onDemo={vi.fn()} />);

    fireEvent.click(screen.getByRole("button", { name: "Enter pairing data manually" }));
    fireEvent.change(screen.getByLabelText("Pairing data"), { target: { value: JSON.stringify(pairingPayload()) } });
    fireEvent.click(screen.getByRole("button", { name: "Review connection" }));

    expect(screen.getByRole("button", { name: "Verify and connect" })).toBeTruthy();
    expect(mocks.pairWithDesktop).not.toHaveBeenCalled();
  });

  it("requires explicit trust before using scanned private HTTP", async () => {
    mocks.scan.mockResolvedValueOnce({ content: JSON.stringify(pairingPayload("private-http")) });
    render(<PairingScreen onPaired={vi.fn()} onDemo={vi.fn()} />);

    fireEvent.click(screen.getByRole("button", { name: "Scan desktop code" }));

    const connect = await screen.findByRole("button", { name: "Verify and connect" }) as HTMLButtonElement;
    expect(connect.disabled).toBe(true);
    expect(screen.getByText("Trust this private HTTP connection")).toBeTruthy();
    expect(mocks.pairWithDesktop).not.toHaveBeenCalled();
  });
});

function renderApp() {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(<QueryClientProvider client={queryClient}><App /></QueryClientProvider>);
}

describe("saved companion credentials", () => {
  it("opens cached projects after an identity network failure", async () => {
    mocks.loadCredential.mockResolvedValueOnce(credential);
    mocks.verifyServerIdentity.mockRejectedValueOnce(new Error("Could not reach the Volum server through the selected network."));
    mocks.listOfflineManifests.mockResolvedValueOnce([manifest]);
    mocks.listOfflineProjectSnapshots.mockResolvedValueOnce([offlineSnapshot]);

    renderApp();

    expect(await screen.findByText("Cached project")).toBeTruthy();
    expect(screen.getByText("Volum Desktop is unreachable. Showing offline copies.")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Try again" })).toBeTruthy();
  });

  it("keeps the identity error screen after a server identity mismatch", async () => {
    mocks.loadCredential.mockResolvedValueOnce(credential);
    mocks.verifyServerIdentity.mockRejectedValueOnce(new CompanionError("That address belongs to a different Volum server.", 0, "server_identity_mismatch"));
    mocks.listOfflineManifests.mockResolvedValueOnce([manifest]);

    renderApp();

    expect(await screen.findByText("Server identity could not be verified")).toBeTruthy();
    expect(screen.queryByText("Cached project")).toBeNull();
    expect(mocks.listOfflineManifests).not.toHaveBeenCalled();
  });

  it("keeps the identity error screen when no offline projects exist", async () => {
    mocks.loadCredential.mockResolvedValueOnce(credential);
    mocks.verifyServerIdentity.mockRejectedValueOnce(new Error("Could not reach the Volum server through the selected network."));

    renderApp();

    expect(await screen.findByText("Server identity could not be verified")).toBeTruthy();
    expect(screen.queryByText("Volum Desktop is unreachable. Showing offline copies.")).toBeNull();
  });

  it("clears a revoked device and returns to pairing", async () => {
    mocks.loadCredential.mockResolvedValueOnce(credential);
    mocks.verifyServerIdentity.mockResolvedValueOnce(credential.baseUrl);
    mocks.requestJson.mockImplementation(async (_credential: Credential, path: string) => {
      if (path === "/api/v1/status") throw new CompanionError("This device is no longer paired.", 401, "unauthorized");
      throw new Error("Unexpected Companion request");
    });

    renderApp();

    expect(await screen.findByText("This device is no longer paired with Volum Desktop")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Pair again" }));

    expect(await screen.findByRole("button", { name: "Scan desktop code" })).toBeTruthy();
    expect(mocks.clearOfflineProjects).toHaveBeenCalledWith(credential.serverId);
    expect(mocks.clearMediaCache).toHaveBeenCalledWith(credential.serverId);
    expect(mocks.clearCredential).toHaveBeenCalledOnce();
  });
});
