export function formatNumberForLocale(
  value: number,
  locale?: string,
  options?: Intl.NumberFormatOptions,
) {
  return new Intl.NumberFormat(locale, options ?? { maximumFractionDigits: value < 10 ? 2 : 0 }).format(value);
}

export function formatBytesBinary(value: number, locale?: string) {
  if (value < 1024) return `${formatNumberForLocale(value, locale)} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let amount = value / 1024;
  let unit = units[0];
  for (let index = 1; amount >= 1024 && index < units.length; index += 1) {
    amount /= 1024;
    unit = units[index];
  }
  return `${formatNumberForLocale(amount, locale, amount < 10
    ? { minimumFractionDigits: 1, maximumFractionDigits: 1 }
    : { maximumFractionDigits: 0 })} ${unit}`;
}

export function formatBytesDecimal(value: number, locale?: string) {
  if (value <= 0) return "0 KB";
  if (value < 1_000_000) return `${Math.max(1, Math.round(value / 1_000))} KB`;
  return `${new Intl.NumberFormat(locale, {
    minimumFractionDigits: value >= 10_000_000 ? 0 : 1,
    maximumFractionDigits: value >= 10_000_000 ? 0 : 1,
  }).format(value / 1_000_000)} MB`;
}

export function formatDuration(seconds: number) {
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.round((seconds % 3600) / 60);
  return hours ? `${hours}h ${minutes}m` : `${minutes}m`;
}
