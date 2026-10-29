import { locale } from "./i18n";

export function formatDate(value: string) {
  return new Intl.DateTimeFormat(locale(), {
    day: "numeric",
    month: "short",
    year: "numeric",
  }).format(new Date(value));
}

export function formatBytes(value: number) {
  if (value <= 0) return "0 KB";
  if (value < 1_000_000) return `${Math.max(1, Math.round(value / 1_000))} KB`;
  return `${(value / 1_000_000).toFixed(value >= 10_000_000 ? 0 : 1)} MB`;
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
  return new Intl.NumberFormat(locale(), {
    maximumFractionDigits: value < 10 ? 2 : 0,
  }).format(value);
}

export function formatDuration(seconds: number) {
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.round((seconds % 3600) / 60);
  return hours ? `${hours}h ${minutes}m` : `${minutes}m`;
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
