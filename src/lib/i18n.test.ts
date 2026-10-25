import { describe, expect, it } from "vitest";
import { localeForLanguage, resolveLanguage, resolveLanguagePreference, translate } from "./i18n";

describe("language resolution", () => {
  it("selects supported base languages from OS locales", () => {
    expect(resolveLanguage("ca-ES")).toBe("ca");
    expect(resolveLanguage("es-ES")).toBe("es");
    expect(resolveLanguage("en-GB")).toBe("en");
  });

  it("falls back to English for unsupported or missing locales", () => {
    expect(resolveLanguage("fr-FR")).toBe("en");
    expect(resolveLanguage()).toBe("en");
  });

  it("uses European Spanish translations and formatting", () => {
    expect(localeForLanguage("es")).toBe("es-ES");
    expect(translate("Settings", "es")).toBe("Ajustes");
    expect(translate("Settings", "ca")).toBe("Configuració");
    expect(translate("Settings", "en")).toBe("Settings");
  });

  it("localizes the first-run choices", () => {
    expect(translate("Use my existing projects", "ca")).toBe("Fes servir els meus projectes");
    expect(translate("Start a new Volum library", "es")).toBe("Crear una biblioteca nueva de Volum");
    expect(translate("Finding your 3D projects…", "es")).toBe("Buscando tus proyectos 3D…");
  });

  it("lets an explicit preference override the system language", () => {
    expect(resolveLanguagePreference("system", "ca-ES")).toBe("ca");
    expect(resolveLanguagePreference("system", "fr-FR")).toBe("en");
    expect(resolveLanguagePreference("es", "ca-ES")).toBe("es");
  });
});
