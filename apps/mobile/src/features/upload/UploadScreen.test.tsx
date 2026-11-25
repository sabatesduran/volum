// @vitest-environment jsdom

import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { open } from "@tauri-apps/plugin-dialog";
import { demoCredential } from "../../lib/demo";
import { UploadScreen } from "./UploadScreen";

vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));
vi.mock("../capture/QuickCapturePanel", () => ({ QuickCapturePanel: () => null }));

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});

it("opens the Android picker without filtering provider MIME types", async () => {
  vi.stubGlobal("navigator", { userAgent: "Android", languages: ["en"] });
  const file = "content://downloads/document/128";
  vi.mocked(open).mockResolvedValue(file);
  const preview = vi.fn().mockResolvedValue(undefined);
  render(<UploadScreen credential={demoCredential} photosEnabled={false} onOpen={vi.fn()} onPreviewStl={preview} />);
  fireEvent.click(screen.getByRole("tab", { name: "3D file" }));
  fireEvent.click(screen.getByRole("button", { name: "Browse 3D files" }));
  expect(open).toHaveBeenCalledWith(expect.objectContaining({ filters: undefined }));
  await waitFor(() => expect(preview).toHaveBeenCalledWith(file, expect.any(String)));
});
