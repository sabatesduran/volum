/** @vitest-environment jsdom */

import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { BackendConnectionStatus } from "../../types";
import { setAppLanguage } from "../../lib/i18n";
import { RemoteConnection } from "./RemoteConnection";

const mocks = vi.hoisted(() => ({
  pendingLibraryMigration: vi.fn(),
  onBackendEvent: vi.fn(),
}));

vi.mock("../../lib/tauri/api", () => ({
  api: {},
  isTauri: () => false,
  pendingLibraryMigration: mocks.pendingLibraryMigration,
  onBackendEvent: mocks.onBackendEvent,
  configureRemoteMirror: vi.fn(),
  connectRemote: vi.fn(),
  forgetRemoteServer: vi.fn(),
  initializeBackend: vi.fn(),
  listMirroredAssets: vi.fn(),
  rollbackLibraryMigration: vi.fn(),
  syncRemoteMirror: vi.fn(),
  useLocalBackend: vi.fn(),
  useRemoteBackend: vi.fn(),
  abandonLibraryMigration: vi.fn(),
  inspectLibraryMigration: vi.fn(),
  resumeLibraryMigration: vi.fn(),
  startLibraryMigration: vi.fn(),
}));

vi.mock("../mirror/useRemoteMirror", () => ({
  useRemoteMirror: () => ({
    busy: false,
    settings: undefined,
    projects: [],
    refresh: vi.fn(),
    changeMode: vi.fn(),
  }),
}));

const localStatus: BackendConnectionStatus = {
  mode: "local",
  configured: false,
  online: true,
  mirrorEnabled: false,
};

describe("remote connection settings", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    setAppLanguage("en");
    mocks.pendingLibraryMigration.mockResolvedValue(undefined);
    mocks.onBackendEvent.mockResolvedValue(() => {});
  });

  afterEach(cleanup);

  it("keeps raw server pairing data behind an explicit dialog", async () => {
    const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(<QueryClientProvider client={queryClient}><RemoteConnection status={localStatus} onChange={vi.fn()} /></QueryClientProvider>);

    expect(screen.queryByRole("textbox", { name: "Server pairing data" })).toBeNull();
    const connectButton = await screen.findByRole("button", { name: /Connect to a Volum server…/ });
    expect(screen.getAllByText("Experimental")).toHaveLength(2);
    fireEvent.click(connectButton);

    expect(screen.getByRole("dialog")).toBeTruthy();
    expect(screen.getByRole("textbox", { name: "Server pairing data" })).toBeTruthy();
  });
});
