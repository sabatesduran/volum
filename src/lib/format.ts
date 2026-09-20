export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let value = bytes / 1024;
  let unit = units[0];
  for (let index = 1; value >= 1024 && index < units.length; index += 1) {
    value /= 1024;
    unit = units[index];
  }
  return `${value < 10 ? value.toFixed(1) : value.toFixed(0)} ${unit}`;
}

export function formatDate(value?: string): string {
  if (!value) return "Never";
  const date = new Date(value);
  const elapsed = Date.now() - date.getTime();
  if (elapsed < 86_400_000) return "Today";
  if (elapsed < 172_800_000) return "Yesterday";
  if (elapsed < 604_800_000) return new Intl.RelativeTimeFormat(undefined, { numeric: "auto" }).format(-Math.round(elapsed / 86_400_000), "day");
  return new Intl.DateTimeFormat(undefined, { month: "short", day: "numeric", year: date.getFullYear() !== new Date().getFullYear() ? "numeric" : undefined }).format(date);
}

export function formatDimensions(dimensions?: [number, number, number]): string {
  if (!dimensions) return "Dimensions unavailable";
  return dimensions.map((value) => Math.round(value)).join(" × ") + " mm";
}

export function formatCurrency(minor: number, currency: string): string {
  return new Intl.NumberFormat(undefined, { style: "currency", currency }).format(minor / 100);
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
