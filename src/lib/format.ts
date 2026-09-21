import { intlLocale, t } from "./i18n";

export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${formatNumber(bytes)} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let value = bytes / 1024;
  let unit = units[0];
  for (let index = 1; value >= 1024 && index < units.length; index += 1) {
    value /= 1024;
    unit = units[index];
  }
  return `${formatNumber(value, value < 10 ? { minimumFractionDigits: 1, maximumFractionDigits: 1 } : { maximumFractionDigits: 0 })} ${unit}`;
}

export function formatDate(value?: string): string {
  if (!value) return t("Never");
  const date = new Date(value);
  const elapsed = Date.now() - date.getTime();
  if (elapsed < 86_400_000) return t("Today");
  if (elapsed < 172_800_000) return t("Yesterday");
  if (elapsed < 604_800_000) return new Intl.RelativeTimeFormat(intlLocale, { numeric: "auto" }).format(-Math.round(elapsed / 86_400_000), "day");
  return new Intl.DateTimeFormat(intlLocale, { month: "short", day: "numeric", year: date.getFullYear() !== new Date().getFullYear() ? "numeric" : undefined }).format(date);
}

export function formatDimensions(dimensions?: [number, number, number]): string {
  if (!dimensions) return t("Dimensions unavailable");
  return dimensions.map((value) => formatNumber(Math.round(value))).join(" × ") + " mm";
}

export function formatCurrency(minor: number, currency: string): string {
  return new Intl.NumberFormat(intlLocale, { style: "currency", currency }).format(minor / 100);
}

export function formatNumber(value: number, options?: Intl.NumberFormatOptions): string {
  return new Intl.NumberFormat(intlLocale, options).format(value);
}

export function titleCase(value: string): string {
  return value ? value[0].toLocaleUpperCase() + value.slice(1).toLocaleLowerCase() : value;
}

export function formatDuration(seconds?: number): string {
  if (seconds == null || !Number.isFinite(seconds) || seconds < 0) return "—";
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.round((seconds % 3600) / 60);
  if (hours === 0) return `${minutes} min`;
  return `${hours} h ${minutes.toString().padStart(2, "0")} min`;
}
