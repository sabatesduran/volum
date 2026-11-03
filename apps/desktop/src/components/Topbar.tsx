import { Search, PanelLeft, X, SlidersHorizontal, Link2, PackageOpen, Plus } from "lucide-react";
import { useAppStore } from "../app/store";
import { t } from "../lib/i18n";
import { useShallow } from "zustand/react/shallow";
import { backendMode } from "../lib/tauri/api";

export function Topbar({ onNewProject, onFilters, onImport, onProjectImport }: { onNewProject?: () => void; onFilters?: () => void; onImport?: () => void; onProjectImport?: () => void }) {
  const { search, setSearch, toggleSidebar, formatFilter, availabilityFilter, tagFilter, dateFrom, dateTo } = useAppStore(useShallow((state) => ({ search: state.search, setSearch: state.setSearch, toggleSidebar: state.toggleSidebar, formatFilter: state.formatFilter, availabilityFilter: state.availabilityFilter, tagFilter: state.tagFilter, dateFrom: state.dateFrom, dateTo: state.dateTo })));
  const activeFilters = [formatFilter, availabilityFilter, tagFilter, dateFrom || dateTo].filter(Boolean).length;
  const remote = backendMode() === "remote";
  return (
    <header className="topbar">
      <button className="icon-button topbar__sidebar" onClick={toggleSidebar} aria-label={t("Toggle sidebar")}><PanelLeft size={18} /></button>
      <div className="search-field">
        <Search size={17} aria-hidden="true" />
        <input
          value={search}
          onChange={(event) => setSearch(event.target.value)}
          placeholder={t("Search projects, folders, collections…")}
          aria-label={t("Search library")}
        />
        {search && <button onClick={() => setSearch("")} aria-label={t("Clear search")}><X size={15} /></button>}
        <kbd>⌘ K</kbd>
      </div>
      <button className="button button--primary topbar__new-project" onClick={onNewProject}><Plus size={16} /><span>{t("New project")}</span></button>
      {!remote && <button className="button button--quiet topbar__import" onClick={onImport}><Link2 size={16} /><span>{t("Import link")}</span></button>}
      {!remote && <button className="button button--quiet topbar__import" onClick={onProjectImport}><PackageOpen size={16} /><span>{t("Import project")}</span></button>}
      <button className={`button button--quiet topbar__filter ${activeFilters ? "is-active" : ""}`} onClick={onFilters}><SlidersHorizontal size={16} /><span>{t("Filter")}{activeFilters ? ` · ${activeFilters}` : ""}</span></button>
    </header>
  );
}
