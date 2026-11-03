export type SettingsSectionId =
  | "general"
  | "library"
  | "remote-library"
  | "materials"
  | "applications"
  | "companion"
  | "backups"
  | "privacy"
  | "advanced"
  | "updates"
  | "about";

export interface SettingsNavigationGroup {
  label: "Everyday" | "Data" | "System";
  sections: Array<{ id: SettingsSectionId; label: string }>;
}

export function settingsNavigation(isRemote: boolean): SettingsNavigationGroup[] {
  return [
    {
      label: "Everyday",
      sections: isRemote
        ? [
          { id: "general", label: "General" },
          { id: "remote-library", label: "Remote library" },
          { id: "materials", label: "Materials" },
          { id: "applications", label: "Applications" },
        ]
        : [
          { id: "general", label: "General" },
          { id: "library", label: "Library" },
          { id: "materials", label: "Materials" },
          { id: "applications", label: "Applications" },
          { id: "companion", label: "Mobile access" },
        ],
    },
    {
      label: "Data",
      sections: isRemote
        ? [{ id: "privacy", label: "Privacy & data" }]
        : [
          { id: "backups", label: "Backups" },
          { id: "privacy", label: "Privacy & data" },
        ],
    },
    {
      label: "System",
      sections: [
        ...(!isRemote ? [{ id: "advanced" as const, label: "Advanced" }] : []),
        { id: "updates", label: "Updates" },
        { id: "about", label: "About" },
      ],
    },
  ];
}

const aliases: Record<string, SettingsSectionId> = {
  appearance: "general",
  libraries: "library",
  "web-imports": "library",
  connection: "advanced",
  developer: "advanced",
  "mobile-access": "companion",
};

export function normalizeSettingsSection(value: string, isRemote: boolean): SettingsSectionId {
  const candidate = aliases[value] ?? value as SettingsSectionId;
  const available = new Set(settingsNavigation(isRemote).flatMap((group) => group.sections.map((section) => section.id)));
  if (available.has(candidate)) return candidate;

  if (isRemote && ["library", "companion", "backups", "advanced"].includes(candidate)) return "remote-library";
  if (!isRemote && candidate === "remote-library") return "advanced";
  return "general";
}
