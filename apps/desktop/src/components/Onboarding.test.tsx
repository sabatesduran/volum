/** @vitest-environment jsdom */

import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { setAppLanguage } from "../lib/i18n";
import { Onboarding } from "./Onboarding";

const mocks = vi.hoisted(() => ({
  addRoot: vi.fn(),
  configureManagedLibrary: vi.fn(),
  prepareRestoreFromPath: vi.fn(),
  startScan: vi.fn(),
  connectRemote: vi.fn()
}));

vi.mock("../lib/tauri/api", () => ({
  isTauri: () => false,
  api: mocks,
  connectRemote: mocks.connectRemote
}));

vi.mock("../features/backups/BackupSettings", () => ({
  RestoreBackupDialog: () => <div>Restore dialog</div>
}));

describe("first-run onboarding", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    setAppLanguage("en");
    mocks.addRoot.mockResolvedValue({ id: "linked-root" });
    mocks.configureManagedLibrary.mockResolvedValue({ id: "managed-root" });
    mocks.startScan.mockResolvedValue(undefined);
  });

  afterEach(cleanup);

  it("makes an existing project folder the primary path", async () => {
    const onComplete = vi.fn();
    render(<Onboarding onComplete={onComplete} />);

    const chooseFolder = screen.getByRole("button", { name: "Choose a project folder" });
    expect(chooseFolder.classList.contains("button--primary")).toBe(true);
    fireEvent.click(chooseFolder);

    await waitFor(() => expect(mocks.addRoot).toHaveBeenCalledWith("/Users/you/3D Projects"));
    expect(mocks.startScan).toHaveBeenCalledWith("linked-root");
    expect(onComplete).toHaveBeenCalledOnce();
  });

  it("offers a clearly separate fresh-library path", async () => {
    const onComplete = vi.fn();
    render(<Onboarding onComplete={onComplete} />);

    fireEvent.click(screen.getByRole("button", { name: "Choose library location" }));

    await waitFor(() => expect(mocks.configureManagedLibrary).toHaveBeenCalledWith("/Users/you/Volum"));
    expect(onComplete).toHaveBeenCalledOnce();
  });

  it("connects a fresh Desktop to a server without creating a local library", async () => {
    const status = { mode: "remote", configured: true, online: true };
    mocks.connectRemote.mockResolvedValue(status);
    const onComplete = vi.fn();
    const onConnected = vi.fn();
    render(<Onboarding onComplete={onComplete} onConnected={onConnected} />);

    fireEvent.click(screen.getByRole("button", { name: "Connect to a Volum server" }));
    const pairButton = screen.getByRole("button", { name: "Pair and connect" });
    expect((pairButton as HTMLButtonElement).disabled).toBe(true);
    fireEvent.change(screen.getByLabelText("Server pairing data"), { target: { value: "pairing-payload" } });
    fireEvent.click(pairButton);

    await waitFor(() => expect(onConnected).toHaveBeenCalledWith(status));
    expect(mocks.connectRemote).toHaveBeenCalledWith("pairing-payload");
    expect(mocks.addRoot).not.toHaveBeenCalled();
    expect(mocks.configureManagedLibrary).not.toHaveBeenCalled();
    expect(onComplete).not.toHaveBeenCalled();
  });

  it("keeps the pairing dialog open with the server error", async () => {
    mocks.connectRemote.mockRejectedValueOnce(new Error("The pairing code has expired."));
    render(<Onboarding onComplete={vi.fn()} onConnected={vi.fn()} />);

    fireEvent.click(screen.getByRole("button", { name: "Connect to a Volum server" }));
    fireEvent.change(screen.getByLabelText("Server pairing data"), { target: { value: "stale" } });
    fireEvent.click(screen.getByRole("button", { name: "Pair and connect" }));

    expect((await screen.findByRole("alert")).textContent).toContain("The pairing code has expired.");
    expect(screen.getByLabelText("Server pairing data")).toBeTruthy();
  });

  it("shows setup failures inline and allows another attempt", async () => {
    mocks.addRoot.mockRejectedValueOnce(new Error("Folder unavailable"));
    render(<Onboarding onComplete={vi.fn()} />);

    fireEvent.click(screen.getByRole("button", { name: "Choose a project folder" }));

    expect((await screen.findByRole("alert")).textContent).toContain("Folder unavailable");
    expect((screen.getByRole("button", { name: "Choose a project folder" }) as HTMLButtonElement).disabled).toBe(false);
  });
});
