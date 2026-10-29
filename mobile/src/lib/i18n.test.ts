import { describe, expect, it } from "vitest";
import { localizedError, localizedErrorFallbackKeys, missingTranslationKeys, setLanguage } from "./i18n";

const modules = import.meta.glob("../**/*.tsx", {
  eager: true,
  import: "default",
  query: "?raw",
}) as Record<string, string>;

const literalKeys = Object.values(modules).flatMap((source) =>
  [...source.matchAll(/\bt\("([^"]+)"/g)].map((match) => match[1]),
);
const dynamicInterfaceKeys = [
  "Library", "Favorites", "Capture", "Collections", "Settings",
  "project", "projects", "file", "files",
  "Remove from favorites", "Add to favorites", "Batch estimate", "Estimated cost",
  "Refreshing", "Pull to refresh",
  "ready", "pending", "failed", "printable", "source", "reference", "version",
  "duplicate", "geometry", "manual", "3mf", "web",
];
const interfaceKeys = [...literalKeys, ...dynamicInterfaceKeys, ...localizedErrorFallbackKeys];

describe("mobile localization", () => {
  it("covers every literal Catalan interface key", () => {
    expect(missingTranslationKeys("ca", interfaceKeys)).toEqual([]);
  });

  it("covers every literal Spanish interface key", () => {
    expect(missingTranslationKeys("es", interfaceKeys)).toEqual([]);
  });

  it("localizes exact and code-based server errors", () => {
    setLanguage("ca");
    expect(localizedError(new Error("Project photos must be non-empty and at most 25 MB.")))
      .toBe("Les fotos de projecte no poden estar buides i han de tenir com a màxim 25 MB.");
    expect(localizedError(Object.assign(new Error("Unknown validation detail"), { code: "invalid_change" })))
      .toBe("El canvi sol·licitat no és vàlid.");
    setLanguage("es");
    expect(localizedError(Object.assign(new Error("Unknown validation detail"), { code: "invalid_upload" })))
      .toBe("La subida seleccionada no es válida.");
    setLanguage("en");
  });
});
