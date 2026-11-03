// @vitest-environment jsdom

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";

const mocks = vi.hoisted(() => ({
  cancel: vi.fn(),
  pairWithDesktop: vi.fn(),
  prepareCredentialStore: vi.fn(),
  requestPermissions: vi.fn(),
  saveCredential: vi.fn(),
  scan: vi.fn(),
}));

vi.mock("@tauri-apps/plugin-barcode-scanner", () => ({
  cancel: mocks.cancel,
  Format: { QRCode: "QRCode" },
  requestPermissions: mocks.requestPermissions,
  scan: mocks.scan,
}));

vi.mock("./lib/credentials", () => ({
  clearCredential: vi.fn(),
  loadCredential: vi.fn(),
  prepareCredentialStore: mocks.prepareCredentialStore,
  saveCredential: mocks.saveCredential,
}));

vi.mock("./lib/transport", async (importOriginal) => ({
  ...await importOriginal<typeof import("./lib/transport")>(),
  pairWithDesktop: mocks.pairWithDesktop,
}));

import { PairingScreen } from "./App";
import type { Credential, PairingPayload } from "./types";

const credential: Credential = {
  serverId: "server-1",
  serverName: "Workshop",
  baseUrl: "http://100.81.102.15:43821",
  deviceId: "device-1",
  token: "device-token",
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
  mocks.pairWithDesktop.mockReset().mockResolvedValue(credential);
  mocks.prepareCredentialStore.mockReset().mockResolvedValue(undefined);
  mocks.requestPermissions.mockReset().mockResolvedValue("granted");
  mocks.saveCredential.mockReset().mockResolvedValue(undefined);
  mocks.scan.mockReset().mockResolvedValue({ content: JSON.stringify(pairingPayload()) });
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
