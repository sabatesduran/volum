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

  it("localizes the optional menu bar setting", () => {
    expect(translate("Menu bar", "ca")).toBe("Barra de menús");
    expect(translate("Show Volum in the menu bar", "es")).toBe("Mostrar Volum en la barra de menús");
  });

  it("localizes project handoff", () => {
    expect(translate("Open on phone", "ca")).toBe("Obre al telèfon");
    expect(translate("Open on phone", "es")).toBe("Abrir en el teléfono");
  });

  it("localizes Photo Room and migration recovery controls", () => {
    expect(translate("Render MP4", "ca")).toBe("Renderitza l'MP4");
    expect(translate("Choose a format", "es")).toBe("Elige un formato");
    expect(translate("Experimental", "ca")).toBe("Experimental");
    expect(translate("Experimental", "es")).toBe("Experimental");
    expect(translate("Abandon migration", "ca")).toBe("Abandona la migració");
    expect(translate("NAS transfer data", "es")).toBe("Datos de transferencia del NAS");
  });

  it("lets an explicit preference override the system language", () => {
    expect(resolveLanguagePreference("system", "ca-ES")).toBe("ca");
    expect(resolveLanguagePreference("system", "fr-FR")).toBe("en");
    expect(resolveLanguagePreference("es", "ca-ES")).toBe("es");
  });
});
