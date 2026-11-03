import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Credential } from "../types";

const mocks = vi.hoisted(() => ({
  appDataDir: vi.fn(),
  invoke: vi.fn(),
  load: vi.fn(),
  loadClient: vi.fn(),
  get: vi.fn(),
  unload: vi.fn(),
}));

vi.mock("@tauri-apps/api/path", () => ({ appDataDir: mocks.appDataDir }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: mocks.invoke }));
vi.mock("@tauri-apps/plugin-stronghold", () => ({ Stronghold: { load: mocks.load } }));

import { clearCredential, loadCredential, saveCredential } from "./credentials";

const credential: Credential = {
  serverId: "server-1",
  serverName: "Workshop",
  baseUrl: "http://100.81.102.15:43821",
  deviceId: "device-1",
  token: "device-token",
};

beforeEach(() => {
  vi.clearAllMocks();
  mocks.appDataDir.mockResolvedValue("/private/app-data/");
  mocks.get.mockResolvedValue(new TextEncoder().encode(JSON.stringify(credential)));
  mocks.loadClient.mockResolvedValue({ getStore: () => ({ get: mocks.get }) });
  mocks.unload.mockResolvedValue(undefined);
  mocks.load.mockResolvedValue({ loadClient: mocks.loadClient, unload: mocks.unload });
});

describe("mobile credential storage", () => {
  it("returns the new vault credential without opening Stronghold", async () => {
    mocks.invoke.mockImplementation(async (command: string) => command === "load_credential"
      ? JSON.stringify(credential)
      : null);

    await expect(loadCredential()).resolves.toEqual(credential);
    expect(mocks.load).not.toHaveBeenCalled();
  });

  it("migrates and verifies a legacy Stronghold credential before cleanup", async () => {
    let stored: string | null = null;
    mocks.invoke.mockImplementation(async (command: string, args?: { credential?: string }) => {
      if (command === "load_credential") return stored;
      if (command === "legacy_credential_store_exists") return true;
      if (command === "vault_password") return "legacy-password";
      if (command === "save_credential") { stored = args?.credential ?? null; return null; }
      if (command === "finalize_legacy_credential_migration") return null;
      throw new Error(`Unexpected command ${command}`);
    });

    await expect(loadCredential()).resolves.toEqual(credential);
    expect(mocks.load).toHaveBeenCalledWith("/private/app-data/credentials.hold", "legacy-password");
    expect(mocks.loadClient).toHaveBeenCalledWith("volum-companion");
    expect(mocks.get).toHaveBeenCalledWith("paired-desktop");
    expect(mocks.unload).toHaveBeenCalledOnce();
    const commands = mocks.invoke.mock.calls.map(([command]) => command);
    expect(commands.indexOf("save_credential")).toBeLessThan(commands.lastIndexOf("load_credential"));
    expect(commands.lastIndexOf("load_credential")).toBeLessThan(commands.indexOf("finalize_legacy_credential_migration"));
  });

  it("keeps the legacy store when its credential is invalid", async () => {
    mocks.get.mockResolvedValue(new TextEncoder().encode("not json"));
    mocks.invoke.mockImplementation(async (command: string) => {
      if (command === "load_credential") return null;
      if (command === "legacy_credential_store_exists") return true;
      if (command === "vault_password") return "legacy-password";
      throw new Error(`Unexpected command ${command}`);
    });

    await expect(loadCredential()).rejects.toThrow("legacy credential store is invalid");
    expect(mocks.unload).toHaveBeenCalledOnce();
    expect(mocks.invoke).not.toHaveBeenCalledWith("finalize_legacy_credential_migration");
  });

  it("clears both current and legacy credential stores when unpairing", async () => {
    mocks.invoke.mockResolvedValue(null);
    await clearCredential();
    expect(mocks.invoke.mock.calls.map(([command]) => command)).toEqual([
      "clear_credential",
      "clear_legacy_credential_store",
    ]);
  });

  it("cleans up a legacy store after a newly paired credential is saved", async () => {
    mocks.invoke.mockImplementation(async (command: string) => command === "legacy_credential_store_exists");
    await saveCredential(credential);
    expect(mocks.invoke.mock.calls.map(([command]) => command)).toEqual([
      "save_credential",
      "legacy_credential_store_exists",
      "finalize_legacy_credential_migration",
    ]);
  });
});
