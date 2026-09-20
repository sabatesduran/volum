import { describe, expect, it } from "vitest";
import { enabledSlicers, parseSlicerConfig } from "./slicers";

describe("slicer preferences", () => {
  it("migrates the previous single-slicer preference", () => {
    const config = parseSlicerConfig({ slicer_path: "/Applications/BambuStudio.app", slicer_name: "Bambu Studio" });
    expect(config.defaultId).toBe("custom-legacy");
    expect(config.customApps[0].path).toBe("/Applications/BambuStudio.app");
  });

  it("only returns enabled and available slicers", () => {
    const applications = [
      { id: "bambu", name: "Bambu", path: "/Bambu.app", installed: true, custom: false, brandColor: "#0a0" },
      { id: "orca", name: "Orca", path: "", installed: false, custom: false, brandColor: "#00a" }
    ];
    expect(enabledSlicers({ enabledIds: ["bambu", "orca"], customApps: [] }, applications).map((app) => app.id)).toEqual(["bambu"]);
  });
});
