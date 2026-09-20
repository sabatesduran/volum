import type { ConfiguredSlicer, SlicerApplication, SlicerConfig } from "../types";

export const emptySlicerConfig = (): SlicerConfig => ({ enabledIds: [], customApps: [] });

export function parseSlicerConfig(preferences: Record<string, string>): SlicerConfig {
  if (preferences.slicer_config) {
    try {
      const parsed = JSON.parse(preferences.slicer_config) as Partial<SlicerConfig>;
      const customApps = Array.isArray(parsed.customApps)
        ? parsed.customApps.filter((app): app is ConfiguredSlicer => Boolean(app?.id && app?.name && app?.path))
        : [];
      const enabledIds = Array.isArray(parsed.enabledIds)
        ? [...new Set(parsed.enabledIds.filter((id): id is string => typeof id === "string"))]
        : [];
      return {
        enabledIds,
        defaultId: typeof parsed.defaultId === "string" ? parsed.defaultId : undefined,
        customApps
      };
    } catch {
      return emptySlicerConfig();
    }
  }
  if (preferences.slicer_path) {
    const legacy: ConfiguredSlicer = {
      id: "custom-legacy",
      name: preferences.slicer_name || "Custom slicer",
      path: preferences.slicer_path
    };
    return { enabledIds: [legacy.id], defaultId: legacy.id, customApps: [legacy] };
  }
  return emptySlicerConfig();
}

export function stringifySlicerConfig(config: SlicerConfig): string {
  return JSON.stringify(config);
}

export function enabledSlicers(config: SlicerConfig, applications: SlicerApplication[]): SlicerApplication[] {
  return config.enabledIds
    .map((id) => applications.find((application) => application.id === id))
    .filter((application): application is SlicerApplication => Boolean(application?.installed && application.path));
}
