export type VolumLanguage = "en" | "ca" | "es";
export type TranslationValues = Readonly<Record<string, string | number>>;
export type TranslationDictionaries<Language extends string = VolumLanguage> = Readonly<Partial<Record<Language, Readonly<Record<string, string>>>>>;

export function resolveLanguage(language?: string): VolumLanguage {
  const code = language?.trim().toLocaleLowerCase().split(/[-_]/)[0];
  return code === "ca" || code === "es" || code === "en" ? code : "en";
}

export function localeForLanguage(language: VolumLanguage): string {
  return language === "ca" ? "ca-ES" : language === "es" ? "es-ES" : "en";
}

export function translate(
  source: string,
  language: string,
  dictionaries: TranslationDictionaries<string>,
  replacements: TranslationValues = {},
): string {
  const translated = dictionaries[language]?.[source] ?? source;
  return translated.replace(/\{([A-Za-z0-9_]+)\}/g, (match, key: string) =>
    replacements[key] == null ? match : String(replacements[key]));
}

export function missingTranslationKeys(
  language: string,
  dictionaries: TranslationDictionaries<string>,
  values: Iterable<string>,
) {
  return [...new Set(values)].filter((value) => !dictionaries[language]?.[value]).sort();
}
