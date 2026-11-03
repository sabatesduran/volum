// @vitest-environment jsdom

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";

const { mocks, MockWebApiError } = vi.hoisted(() => {
  class WebApiError extends Error {
    constructor(public status: number, message: string) { super(message); }
  }
  return {
    MockWebApiError: WebApiError,
    mocks: {
      bootstrap: vi.fn(),
      clearCredential: vi.fn(),
      createMigrationPairing: vi.fn(),
      createPairing: vi.fn(),
      listDevices: vi.fn(),
      loadCredential: vi.fn(),
      login: vi.fn(),
      logout: vi.fn(),
      revokeDevice: vi.fn(),
      saveCredential: vi.fn(),
      toDataURL: vi.fn(),
    },
  };
});

vi.mock("./api", () => ({
  bootstrap: mocks.bootstrap,
  createMigrationPairing: mocks.createMigrationPairing,
  createPairing: mocks.createPairing,
  listDevices: mocks.listDevices,
  login: mocks.login,
  logout: mocks.logout,
  revokeDevice: mocks.revokeDevice,
  WebApiError: MockWebApiError,
}));

vi.mock("./credential", () => ({
  clearCredential: mocks.clearCredential,
  loadCredential: mocks.loadCredential,
  saveCredential: mocks.saveCredential,
}));

vi.mock("qrcode", () => ({ default: { toDataURL: mocks.toDataURL } }));

import { App } from "./main";

const host = {
  configured: true,
  serverId: "server-1",
  serverName: "Workshop",
  publicUrl: "https://volum.example.com",
  secureTransport: true,
  protocolVersion: 1,
};
const credential = {
  serverId: "server-1",
  serverName: "Workshop",
  deviceId: "admin-1",
  token: "admin-token",
};
const devices = [
  { id: "phone-1", name: "Workshop phone", platform: "iOS", role: "client", pairedAt: "2026-11-01T10:00:00Z" },
  { id: "admin-1", name: "Safari browser", platform: "web", role: "admin", pairedAt: "2026-11-01T10:00:00Z" },
];

beforeEach(() => {
  vi.clearAllMocks();
  mocks.bootstrap.mockResolvedValue(host);
  mocks.loadCredential.mockResolvedValue(undefined);
  mocks.login.mockResolvedValue({ ...credential, protocolVersion: 1 });
  mocks.saveCredential.mockResolvedValue(undefined);
  mocks.clearCredential.mockResolvedValue(undefined);
  mocks.listDevices.mockResolvedValue(devices);
  mocks.createPairing.mockResolvedValue({
    serverId: "server-1",
    serverName: "Workshop",
    baseUrl: host.publicUrl,
    secret: "pairing-secret",
    expiresAt: "2999-01-01T00:00:00Z",
    transport: "https",
    qrPayload: "pairing-payload",
  });
  mocks.toDataURL.mockResolvedValue("data:image/png;base64,qr");
  mocks.revokeDevice.mockResolvedValue(undefined);
  vi.spyOn(window, "confirm").mockReturnValue(true);
  Object.defineProperty(navigator, "clipboard", {
    configurable: true,
    value: { writeText: vi.fn().mockResolvedValue(undefined) },
  });
});

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

describe("server administrator", () => {
  it("signs in and persists a separate browser credential", async () => {
    render(<App />);
    await screen.findByRole("heading", { name: "Sign in to Workshop" });

    fireEvent.change(screen.getByLabelText("Password"), { target: { value: "correct horse battery staple" } });
    fireEvent.click(screen.getByRole("button", { name: "Sign in" }));

    await screen.findByRole("heading", { name: "Trusted devices" });
    expect(mocks.login).toHaveBeenCalledWith("correct horse battery staple", expect.stringContaining("browser"));
    expect(mocks.saveCredential).toHaveBeenCalledWith(expect.objectContaining({ token: "admin-token" }));
    expect(await screen.findByText("Workshop phone")).toBeTruthy();
  });

  it("creates a one-use pairing dialog and copies its payload", async () => {
    mocks.loadCredential.mockResolvedValue(credential);
    render(<App />);
    await screen.findByRole("heading", { name: "Trusted devices" });
    await screen.findByText("Workshop phone");

    fireEvent.click(screen.getByRole("button", { name: "Pair device" }));
    await screen.findByRole("dialog", { name: "Pair a device" });
    expect(mocks.createPairing).toHaveBeenCalledWith(credential);
    expect(mocks.toDataURL).toHaveBeenCalledWith("pairing-payload", expect.any(Object));

    fireEvent.click(screen.getByRole("button", { name: "Copy pairing data" }));
    await waitFor(() => expect(navigator.clipboard.writeText).toHaveBeenCalledWith("pairing-payload"));
    expect((await screen.findByRole("status")).textContent).toContain("Pairing data copied");
  });

  it("revokes a paired device after confirmation and refreshes the list", async () => {
    mocks.loadCredential.mockResolvedValue(credential);
    render(<App />);
    await screen.findByText("Workshop phone");

    fireEvent.click(screen.getByRole("button", { name: "Revoke" }));
    await waitFor(() => expect(mocks.revokeDevice).toHaveBeenCalledWith(credential, "phone-1"));
    expect(window.confirm).toHaveBeenCalledWith("Revoke access for “Workshop phone”?");
    expect(mocks.listDevices).toHaveBeenCalledTimes(2);
  });

  it("clears an expired browser credential and returns to sign in", async () => {
    mocks.loadCredential.mockResolvedValue(credential);
    mocks.listDevices.mockRejectedValue(new MockWebApiError(401, "Expired"));
    render(<App />);

    await screen.findByRole("heading", { name: "Sign in to Workshop" });
    expect(mocks.clearCredential).toHaveBeenCalledOnce();
  });
});
