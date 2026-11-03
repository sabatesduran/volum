import { useEffect, useState } from "react";
import {
  missingTranslationKeys as sharedMissingTranslationKeys,
  translate,
  type TranslationDictionaries,
} from "@volum/shared/i18n";
import {
  DEFAULT_LANGUAGE,
  MOBILE_LANGUAGES,
  type AppLanguage,
  type MobileLanguageDefinition,
} from "../locales/generated";

export type { AppLanguage } from "../locales/generated";

const localeModules = import.meta.glob("../locales/*.json", {
  eager: true,
  import: "default",
}) as Record<string, Record<string, string>>;

const dictionaries = Object.fromEntries(MOBILE_LANGUAGES.map(({ code }) => {
  const dictionary = localeModules[`../locales/${code}.json`];
  if (!dictionary) throw new Error(`Missing mobile translation dictionary for ${code}`);
  return [code, dictionary];
})) as TranslationDictionaries<AppLanguage>;

const errorFallbacks: Record<string, string> = {
  unauthorized: "Pair this device with Volum Desktop.",
  invalid_device: "Enter valid device details.",
  pairing_expired: "Create a new pairing code on Volum Desktop.",
  pairing_failed: "Volum could not save this paired device.",
  organization_writes_disabled: "Enable mobile organization changes in Volum Desktop settings first.",
  photo_uploads_disabled: "Enable mobile photo uploads in the canonical Volum host settings first.",
  edit_conflict: "This project changed on another device. Refresh and try again.",
  not_found: "The requested Volum item is unavailable.",
  invalid_request: "The request is invalid.",
  invalid_change: "The requested change is invalid.",
  invalid_upload: "The selected upload is invalid.",
  upload_size_required: "The upload size is required.",
  photo_too_large: "Project photos must be non-empty and at most 25 MB.",
  upload_busy: "Another Companion upload is being imported. Try again shortly.",
  viewer_unavailable: "The model viewer is temporarily unavailable.",
  viewer_mesh_too_large: "This model is too detailed for the mobile viewer.",
  upload_too_large: "3D files can be at most 500 MB.",
  upload_failed: "Volum Desktop could not import the uploaded file.",
  managed_library_required: "Configure a managed library on Volum Desktop before uploading files.",
  upload_target_unavailable: "Choose a managed project that is not in Trash.",
  upload_write_failed: "Volum Desktop could not finish saving this upload.",
  request_failed: "Volum could not complete this request.",
  invalid_response: "Volum Desktop returned an unreadable response",
  connection_failed: "Could not reach the Volum server through the selected network.",
};

export const localizedErrorFallbackKeys = Object.values(errorFallbacks);

function normalizedTag(value?: string | null) {
  if (!value) return undefined;
  try {
    return new Intl.Locale(value.replaceAll("_", "-")).toString().toLocaleLowerCase();
  } catch {
    return value.trim().replaceAll("_", "-").toLocaleLowerCase() || undefined;
  }
}

export function resolveLanguage(value?: string | null): AppLanguage {
  const tag = normalizedTag(value);
  if (!tag) return DEFAULT_LANGUAGE;
  const exact = MOBILE_LANGUAGES.find((candidate) => [
    candidate.code,
    candidate.locale,
    candidate.intl,
    candidate.ios,
    candidate.android,
    ...candidate.aliases,
  ].some((candidateTag) => normalizedTag(candidateTag) === tag));
  if (exact) return exact.code;
  const base = tag.split("-")[0];
  return MOBILE_LANGUAGES.find((candidate) => candidate.locale.split("-")[0].toLocaleLowerCase() === base)?.code
    ?? DEFAULT_LANGUAGE;
}

function browserLanguage() {
  if (typeof navigator === "undefined") return DEFAULT_LANGUAGE;
  return navigator.languages?.[0] ?? navigator.language;
}

let language = resolveLanguage(browserLanguage());

export function languageDefinition(value = language): MobileLanguageDefinition {
  return MOBILE_LANGUAGES.find((candidate) => candidate.code === value)
    ?? MOBILE_LANGUAGES.find((candidate) => candidate.code === DEFAULT_LANGUAGE)
    ?? MOBILE_LANGUAGES[0];
}

export function t(value: string, parameters?: Record<string, string | number>) {
  return translate(value, language, dictionaries, parameters);
}

export function localizedError(reason: unknown) {
  const message = reason instanceof Error ? reason.message : String(reason);
  const direct = t(message);
  if (direct !== message || language === DEFAULT_LANGUAGE) return direct;
  const rejectedUpload = message.match(/^Volum Desktop rejected this upload \((\d+)\)$/);
  if (rejectedUpload) return t("Volum Desktop rejected this upload ({status})", { status: rejectedUpload[1] });
  const returnedStatus = message.match(/^Volum Desktop returned (\d+)$/);
  if (returnedStatus) return t("Volum Desktop returned {status}", { status: returnedStatus[1] });
  const code = typeof reason === "object" && reason && "code" in reason
    ? String((reason as { code?: unknown }).code ?? "")
    : "";
  return errorFallbacks[code] ? t(errorFallbacks[code]) : message;
}

export function locale() {
  return languageDefinition().intl;
}

function applyDocumentLanguage() {
  if (typeof document === "undefined") return;
  const definition = languageDefinition();
  document.documentElement.lang = definition.locale;
  document.documentElement.dir = definition.direction;
}

export function setLanguage(next: AppLanguage) {
  language = next;
  applyDocumentLanguage();
  if (typeof window !== "undefined") {
    window.dispatchEvent(new CustomEvent("volum-language-change", { detail: next }));
  }
}

export function setLanguageFromLocale(localeTag?: string | null) {
  setLanguage(resolveLanguage(localeTag));
}

export function missingTranslationKeys(next: AppLanguage, values: Iterable<string>) {
  return sharedMissingTranslationKeys(next, dictionaries, values);
}

export function useLanguage() {
  const [current, setCurrent] = useState(language);
  useEffect(() => {
    const update = (event: Event) => setCurrent((event as CustomEvent<AppLanguage>).detail);
    window.addEventListener("volum-language-change", update);
    return () => window.removeEventListener("volum-language-change", update);
  }, []);
  return current;
}

applyDocumentLanguage();
