import { invoke } from "@tauri-apps/api/core";
import { openAppSettings } from "@tauri-apps/plugin-barcode-scanner";
import { resolveLanguage, setLanguage } from "./i18n";

interface AppLocaleResponse {
  locale: string;
}

function browserLocale() {
  return navigator.languages?.[0] ?? navigator.language ?? "en";
}

export async function currentAppLocale() {
  try {
    const response = await invoke<AppLocaleResponse>("plugin:app-locale|get_app_locale");
    return response.locale || browserLocale();
  } catch {
    // Browser previews and older native builds have no app-locale plugin.
    return browserLocale();
  }
}

export async function synchronizeAppLanguage() {
  setLanguage(resolveLanguage(await currentAppLocale()));
}

export function installAppLanguageSynchronization() {
  const synchronize = () => { void synchronizeAppLanguage(); };
  const onVisibilityChange = () => {
    if (document.visibilityState === "visible") synchronize();
  };
  window.addEventListener("focus", synchronize);
  document.addEventListener("visibilitychange", onVisibilityChange);
  return () => {
    window.removeEventListener("focus", synchronize);
    document.removeEventListener("visibilitychange", onVisibilityChange);
  };
}

export async function openLanguageSettings() {
  try {
    await invoke("plugin:app-locale|open_language_settings");
  } catch {
    // Keep browser previews and native builds made before the plugin usable.
    await openAppSettings();
  }
}
