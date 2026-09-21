import { Check, X } from "lucide-react";
import { useQuery } from "@tanstack/react-query";
import { useAppStore } from "../app/store";
import { api } from "../lib/tauri/api";
import { t } from "../lib/i18n";

const formats = ["", "stl", "3mf", "obj", "step"];

export function FilterPanel({ onClose }: { onClose: () => void }) {
  const { data: tags = [] } = useQuery({ queryKey: ["tags"], queryFn: api.tags });
  const { formatFilter, setFormatFilter, availabilityFilter, setAvailabilityFilter, tagFilter, setTagFilter, dateField, setDateField, dateFrom, dateTo, setDateRange, clearFilters } = useAppStore();
  const hasFilters = Boolean(formatFilter || availabilityFilter || tagFilter || dateFrom || dateTo);
  return (
    <div className="filter-panel">
      <header><strong>{t("Quick filters")}</strong><button className="icon-button icon-button--tiny" onClick={onClose} aria-label={t("Close")}><X size={15} /></button></header>
      <fieldset><legend>{t("Format")}</legend><div className="filter-options">{formats.map((format) => <button key={format || "all"} className={formatFilter === format ? "is-active" : ""} onClick={() => setFormatFilter(format)}>{formatFilter === format && <Check size={12} />}{format ? format.toUpperCase() : t("All formats")}</button>)}</div></fieldset>
      <fieldset><legend>{t("Availability")}</legend><div className="filter-options">{([['', "Everything"], ["available", "Available"], ["offline", "Offline"]] as const).map(([value, label]) => <button key={label} className={availabilityFilter === value ? "is-active" : ""} onClick={() => setAvailabilityFilter(value)}>{availabilityFilter === value && <Check size={12} />}{t(label)}</button>)}</div></fieldset>
      {tags.length > 0 && <fieldset><legend>{t("Tag")}</legend><label className="filter-select"><select value={tagFilter} onChange={(event) => setTagFilter(event.target.value)}><option value="">{t("All tags")}</option>{tags.map((tag) => <option value={tag.id} key={tag.id}>{tag.name} ({tag.modelCount})</option>)}</select></label></fieldset>}
      <fieldset><legend>{t("Date")}</legend><div className="segmented filter-date-field"><button className={dateField === "modified" ? "is-active" : ""} onClick={() => setDateField("modified")}>{t("Modified")}</button><button className={dateField === "added" ? "is-active" : ""} onClick={() => setDateField("added")}>{t("Added")}</button></div><div className="filter-date-range"><label><span>{t("From")}</span><input type="date" value={dateFrom} max={dateTo || undefined} onChange={(event) => setDateRange(event.target.value, dateTo)} /></label><label><span>{t("To")}</span><input type="date" value={dateTo} min={dateFrom || undefined} onChange={(event) => setDateRange(dateFrom, event.target.value)} /></label></div></fieldset>
      {hasFilters && <button className="button button--quiet button--full" onClick={clearFilters}>{t("Clear filters")}</button>}
    </div>
  );
}
