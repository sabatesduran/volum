/** @vitest-environment jsdom */

import { render, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useAppStore } from "../app/store";
import type { ModelSummary } from "../types";
import { ModelGrid } from "./ModelGrid";

vi.mock("@tanstack/react-virtual", () => ({
  useVirtualizer: (options: { count: number }) => ({
    measure: vi.fn(),
    getTotalSize: () => options.count * 300,
    getVirtualItems: () => options.count > 0 ? [{ index: 0, key: "row-0", start: 0 }] : []
  })
}));

class ResizeObserverStub {
  observe() {}
  disconnect() {}
}

const models: ModelSummary[] = Array.from({ length: 4 }, (_, index) => ({
  id: `model-${index}`,
  displayName: `Model ${index}`,
  folderId: "folder",
  folderName: "Models",
  relativeFolder: "",
  primaryExtension: "stl",
  favorite: false,
  addedAt: "2026-01-01T00:00:00Z",
  modifiedAt: "2026-01-01T00:00:00Z",
  missing: false,
  assetCount: 1,
  bundleMode: "automatic"
}));

describe("model grid layout", () => {
  beforeEach(() => {
    vi.stubGlobal("ResizeObserver", ResizeObserverStub);
    useAppStore.setState({ density: "comfortable" });
    Object.defineProperty(HTMLElement.prototype, "clientWidth", {
      configurable: true,
      get() {
        return (this as HTMLElement).classList.contains("virtual-grid") ? 1620 : 0;
      }
    });
  });

  it("measures the real grid after the loading skeleton is replaced", async () => {
    const view = render(<ModelGrid models={[]} loading />);

    view.rerender(<ModelGrid models={models} />);

    await waitFor(() => {
      const row = view.container.querySelector<HTMLElement>(".virtual-grid__row");
      expect(row).not.toBeNull();
      expect(row?.style.gridTemplateColumns).toBe("repeat(6, minmax(0, 1fr))");
    });
  });
});
