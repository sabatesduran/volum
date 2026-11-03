/** @vitest-environment jsdom */

import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { setAppLanguage } from "../lib/i18n";
import { Onboarding } from "./Onboarding";

const mocks = vi.hoisted(() => ({
  addRoot: vi.fn(),
  configureManagedLibrary: vi.fn(),
  prepareRestoreFromPath: vi.fn(),
  startScan: vi.fn()
}));

vi.mock("../lib/tauri/api", () => ({
  isTauri: () => false,
  api: mocks
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

  it("shows setup failures inline and allows another attempt", async () => {
    mocks.addRoot.mockRejectedValueOnce(new Error("Folder unavailable"));
    render(<Onboarding onComplete={vi.fn()} />);

    fireEvent.click(screen.getByRole("button", { name: "Choose a project folder" }));

    expect((await screen.findByRole("alert")).textContent).toContain("Folder unavailable");
    expect((screen.getByRole("button", { name: "Choose a project folder" }) as HTMLButtonElement).disabled).toBe(false);
  });
});
