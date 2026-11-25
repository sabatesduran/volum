/** @vitest-environment jsdom */

import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { setAppLanguage } from "../../lib/i18n";
import type { CompanionStatus } from "../../types";
import { CompanionSettings } from "./CompanionSettings";

const mocks = vi.hoisted(() => ({
  companionStatus: vi.fn(),
  beginCompanionTailnetSignIn: vi.fn(),
  openExternal: vi.fn(),
  setCompanionNetwork: vi.fn(),
}));

vi.mock("../../lib/tauri/api", () => ({ api: mocks }));

const status: CompanionStatus = {
  enabled: false, running: false, serverId: "server", port: 80, devices: [],
  organizationWrites: false, photoUploads: false, networkMode: "embedded-tailscale",
  tailnetAvailable: true, tailnetState: "signed-out", tailnetHostname: "volum",
};

async function openSettings() {
  const client = new QueryClient();
  render(<QueryClientProvider client={client}><CompanionSettings /></QueryClientProvider>);
  await screen.findByRole("button", { name: "Sign in to Tailscale" });
  return client;
}

describe("Companion Tailscale setup", () => {
  beforeEach(() => {
    vi.resetAllMocks();
    setAppLanguage("en");
    mocks.companionStatus.mockResolvedValue(status);
    mocks.openExternal.mockResolvedValue(undefined);
  });

  afterEach(cleanup);

  it("opens and can reopen the Tailscale browser approval URL", async () => {
    const tailnetAuthUrl = "https://login.tailscale.com/a/example";
    mocks.beginCompanionTailnetSignIn.mockResolvedValue({
      ...status, enabled: true, tailnetState: "needs-login", tailnetAuthUrl,
    });
    await openSettings();
    fireEvent.click(screen.getByRole("button", { name: "Sign in to Tailscale" }));
    const reopen = await screen.findByRole("button", { name: "Open Tailscale sign-in" });
    expect(mocks.beginCompanionTailnetSignIn).toHaveBeenCalledWith("volum", undefined);
    expect(mocks.openExternal).toHaveBeenCalledWith(tailnetAuthUrl);
    fireEvent.click(reopen);
    expect(mocks.openExternal).toHaveBeenCalledTimes(2);
  });

  it("submits an auth key without opening the browser", async () => {
    mocks.beginCompanionTailnetSignIn.mockResolvedValue({
      ...status, enabled: true, running: true, tailnetState: "connected",
    });
    const key = "tskey-auth-example";
    await openSettings();
    const input = screen.getByLabelText("Tailscale auth key") as HTMLInputElement;
    fireEvent.change(input, { target: { value: key } });
    fireEvent.click(screen.getByRole("button", { name: "Connect using auth key" }));
    expect(input.value).toBe("");
    await screen.findByText("Available on your selected network");
    expect(mocks.beginCompanionTailnetSignIn).toHaveBeenCalledWith("volum", key);
    expect(mocks.openExternal).not.toHaveBeenCalled();
  });

  it("keeps manual network edits when status is refreshed", async () => {
    const client = await openSettings();
    fireEvent.click(screen.getByRole("radio", { name: /Custom/ }));
    fireEvent.change(screen.getByLabelText("Listen on this private IP"), { target: { value: "192.168.1.20" } });
    fireEvent.change(screen.getByLabelText("Address clients will use"), { target: { value: "http://192.168.1.20:43821" } });
    await act(async () => { client.setQueryData(["companion-status"], { ...status, enabled: true }); });
    fireEvent.click(screen.getByRole("button", { name: "Save network settings" }));
    await waitFor(() => expect(mocks.setCompanionNetwork).toHaveBeenCalledWith("custom", "192.168.1.20", "http://192.168.1.20:43821", undefined));
  });
});
