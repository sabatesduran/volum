import { Search, PanelLeft, X, SlidersHorizontal, Link2 } from "lucide-react";
import { useAppStore } from "../app/store";
import { t } from "../lib/i18n";

export function Topbar({ onFilters, onImport }: { onFilters?: () => void; onImport?: () => void }) {
  const { search, setSearch, toggleSidebar, formatFilter, availabilityFilter, tagFilter, dateFrom, dateTo } = useAppStore();
  const activeFilters = [formatFilter, availabilityFilter, tagFilter, dateFrom || dateTo].filter(Boolean).length;
  return (
    <header className="topbar">
      <button className="icon-button topbar__sidebar" onClick={toggleSidebar} aria-label={t("Toggle sidebar")}><PanelLeft size={18} /></button>
      <div className="search-field">
        <Search size={17} aria-hidden="true" />
        <input
          value={search}
          onChange={(event) => setSearch(event.target.value)}
          placeholder={t("Search models, folders, collections…")}
          aria-label={t("Search library")}
        />
        {search && <button onClick={() => setSearch("")} aria-label={t("Clear search")}><X size={15} /></button>}
        <kbd>⌘ K</kbd>
      </div>
      <button className="button button--quiet topbar__import" onClick={onImport}><Link2 size={16} /><span>{t("Import link")}</span></button>
      <button className={`button button--quiet topbar__filter ${activeFilters ? "is-active" : ""}`} onClick={onFilters}><SlidersHorizontal size={16} /><span>{t("Filter")}{activeFilters ? ` · ${activeFilters}` : ""}</span></button>
    </header>
  );
}
