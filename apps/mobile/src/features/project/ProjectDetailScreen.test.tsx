// @vitest-environment jsdom

import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";

const mocks = vi.hoisted(() => ({
  getOfflineProject: vi.fn(),
  requestJson: vi.fn(),
}));

vi.mock("../../lib/offline", () => ({
  downloadOfflineProject: vi.fn(),
  getOfflineProject: mocks.getOfflineProject,
  removeOfflineProject: vi.fn(),
}));

vi.mock("../../lib/transport", async (importOriginal) => ({
  ...await importOriginal<typeof import("../../lib/transport")>(),
  requestJson: mocks.requestJson,
}));

vi.mock("./ProjectMedia", () => ({ ProjectImage: () => null }));

import { ProjectDetailScreen } from "./ProjectDetailScreen";
import type { Credential } from "../../types";

const credential: Credential = {
  serverId: "server-1",
  serverName: "Workshop",
  baseUrl: "http://192.168.1.20:43821",
  deviceId: "device-1",
  token: "device-token",
};

afterEach(cleanup);

describe("project detail", () => {
  it("keeps loading a freshly saved project that has no offline copy", async () => {
    mocks.getOfflineProject.mockResolvedValue(undefined);
    mocks.requestJson.mockReturnValue(new Promise(() => undefined));
    const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });

    render(<QueryClientProvider client={queryClient}><ProjectDetailScreen credential={credential} id="project-1" canWrite onBack={vi.fn()} onOpen={vi.fn()} /></QueryClientProvider>);

    await vi.waitFor(() => expect(mocks.getOfflineProject).toHaveBeenCalled());
    await new Promise((resolve) => setTimeout(resolve, 20));
    expect(screen.getByRole("status", { name: "Loading project" })).toBeTruthy();
    expect(screen.queryByText("Could not load Volum")).toBeNull();
  });
});
