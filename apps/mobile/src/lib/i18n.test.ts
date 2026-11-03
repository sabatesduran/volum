import { describe, expect, it } from "vitest";
import { photoRoomTranslations } from "@volum/photo-room/i18n";
import { DEFAULT_LANGUAGE, MOBILE_LANGUAGES } from "../locales/generated";
import {
  localizedError,
  localizedErrorFallbackKeys,
  missingTranslationKeys,
  resolveLanguage,
  setLanguage,
} from "./i18n";

const modules = import.meta.glob("../**/*.tsx", {
  eager: true,
  import: "default",
  query: "?raw",
}) as Record<string, string>;
const dictionaries = import.meta.glob("../locales/*.json", {
  eager: true,
  import: "default",
}) as Record<string, Record<string, string>>;

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
const interfaceKeys = [
  ...literalKeys,
  ...dynamicInterfaceKeys,
  ...localizedErrorFallbackKeys,
  ...Object.keys(photoRoomTranslations.ca),
  "Connect to Volum Desktop before adding this photo to the project.",
];

function placeholders(value: string) {
  return [...value.matchAll(/\{([A-Za-z0-9_]+)\}/g)].map((match) => match[1]).sort();
}

describe("mobile localization", () => {
  for (const language of MOBILE_LANGUAGES) {
    it(`covers every ${language.nativeName} interface key`, () => {
      expect(missingTranslationKeys(language.code, interfaceKeys)).toEqual([]);
    });
  }

  it("keeps interpolation placeholders aligned in every translation", () => {
    const english = dictionaries["../locales/en.json"];
    for (const language of MOBILE_LANGUAGES) {
      if (language.code === DEFAULT_LANGUAGE) continue;
      const translated = dictionaries[`../locales/${language.code}.json`];
      for (const [key, value] of Object.entries(english)) {
        expect(placeholders(value), `${language.code}: ${key}`).toEqual(placeholders(translated[key] ?? ""));
      }
    }
  });

  it("resolves app-level BCP-47 locales", () => {
    expect(resolveLanguage("ca-ES")).toBe("ca");
    expect(resolveLanguage("es-MX")).toBe("es");
    expect(resolveLanguage("en-GB")).toBe("en");
    expect(resolveLanguage("fr-FR")).toBe("en");
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
