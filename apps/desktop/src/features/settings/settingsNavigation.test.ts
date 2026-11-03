import { describe, expect, it } from "vitest";
import { normalizeSettingsSection, settingsNavigation } from "./settingsNavigation";

describe("settings navigation", () => {
  it("starts local settings with everyday preferences and keeps infrastructure advanced", () => {
    const groups = settingsNavigation(false);
    expect(groups[0].sections.map((section) => section.id)).toEqual([
      "general", "library", "materials", "applications", "companion",
    ]);
    expect(groups[2].sections.map((section) => section.id)).toContain("advanced");
  });

  it("promotes remote library controls only when the Desktop uses a server", () => {
    const sections = settingsNavigation(true).flatMap((group) => group.sections.map((section) => section.id));
    expect(sections.slice(0, 2)).toEqual(["general", "remote-library"]);
    expect(sections).not.toContain("library");
    expect(sections).not.toContain("companion");
    expect(sections).not.toContain("backups");
  });

  it("keeps old hashes useful and redirects unavailable local sections", () => {
    expect(normalizeSettingsSection("appearance", false)).toBe("general");
    expect(normalizeSettingsSection("web-imports", false)).toBe("library");
    expect(normalizeSettingsSection("connection", false)).toBe("advanced");
    expect(normalizeSettingsSection("connection", true)).toBe("remote-library");
    expect(normalizeSettingsSection("backups", true)).toBe("remote-library");
    expect(normalizeSettingsSection("unknown", false)).toBe("general");
  });
});
