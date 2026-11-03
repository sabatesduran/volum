/** @vitest-environment jsdom */

import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ProjectQrDialog } from "./ProjectQrDialog";

const { toDataURL } = vi.hoisted(() => ({
  toDataURL: vi.fn(async () => "data:image/png;base64,project-qr"),
}));

vi.mock("qrcode", () => ({ default: { toDataURL } }));

describe("desktop project QR dialog", () => {
  beforeEach(() => toDataURL.mockClear());

  it("renders the mobile-compatible project payload", async () => {
    render(<ProjectQrDialog serverId="server-123" projectId="project-456" projectName="Tool holder" readiness="ready" onClose={vi.fn()} />);

    await waitFor(() => expect(toDataURL).toHaveBeenCalledWith(
      "volum://project/server-123/project-456",
      expect.objectContaining({ errorCorrectionLevel: "M" }),
    ));
    expect(await screen.findByAltText("Project QR code for Tool holder")).toBeTruthy();
  });

  it("sends an unpaired user to Companion Access without creating a dead QR", () => {
    const openCompanion = vi.fn();
    render(<ProjectQrDialog serverId="server-123" projectId="project-456" projectName="Tool holder" readiness="unpaired" onClose={vi.fn()} onOpenCompanion={openCompanion} />);

    expect(toDataURL).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Open Companion Access" }));
    expect(openCompanion).toHaveBeenCalledOnce();
  });
});
