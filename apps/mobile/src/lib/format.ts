import { locale } from "./i18n";
import { formatBytesDecimal, formatDuration as sharedFormatDuration, formatNumberForLocale } from "@volum/shared/format";

export function formatDate(value: string) {
  return new Intl.DateTimeFormat(locale(), {
    day: "numeric",
    month: "short",
    year: "numeric",
  }).format(new Date(value));
}

export function formatBytes(value: number) {
  return formatBytesDecimal(value, locale());
}

export function formatDateTime(value: string) {
  return new Intl.DateTimeFormat(locale(), {
    day: "numeric",
    month: "short",
    hour: "2-digit",
    minute: "2-digit",
  }).format(new Date(value));
}

export function formatNumber(value: number) {
  return formatNumberForLocale(value, locale(), {
    maximumFractionDigits: value < 10 ? 2 : 0,
  });
}

export function formatDuration(seconds: number) {
  return sharedFormatDuration(seconds);
}

export function formatArea(value: number) {
  return value >= 1_000_000
    ? `${formatNumber(value / 1_000_000)} m²`
    : `${formatNumber(value / 100)} cm²`;
}

export function formatVolume(value: number) {
  return value >= 1_000_000
    ? `${formatNumber(value / 1_000_000)} L`
    : `${formatNumber(value / 1_000)} cm³`;
}

export function formatMoney(value?: number, currency = "EUR") {
  if (value == null) return "—";
  try {
    return new Intl.NumberFormat(locale(), { style: "currency", currency }).format(value / 100);
  } catch {
    return `${(value / 100).toFixed(2)} ${currency}`;
  }
}
