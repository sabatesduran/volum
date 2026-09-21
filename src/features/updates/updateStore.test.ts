import { beforeEach, describe, expect, it } from "vitest";
import { checkForUpdates, updatePercent, useUpdateStore } from "./updateStore";

beforeEach(() => {
  useUpdateStore.setState({
    phase: "idle",
    update: undefined,
    downloadedBytes: 0,
    totalBytes: undefined,
    downloadReady: false,
    error: undefined,
    checkedAt: undefined,
    dismissedVersion: undefined
  });
});

describe("update progress", () => {
  it("calculates and caps known download progress", () => {
    expect(updatePercent(25, 100)).toBe(25);
    expect(updatePercent(120, 100)).toBe(100);
  });

  it("uses indeterminate progress when the server omits a total", () => {
    expect(updatePercent(25)).toBeUndefined();
    expect(updatePercent(25, 0)).toBeUndefined();
  });

  it("reports manual checks as previews outside Tauri", async () => {
    await checkForUpdates(true);
    expect(useUpdateStore.getState().phase).toBe("preview");
    expect(useUpdateStore.getState().checkedAt).toEqual(expect.any(Number));
  });
});
