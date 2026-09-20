import { Search, PanelLeft, X, SlidersHorizontal, Link2 } from "lucide-react";
import { useAppStore } from "../app/store";

export function Topbar({ onFilters, onImport }: { onFilters?: () => void; onImport?: () => void }) {
  const { search, setSearch, toggleSidebar, formatFilter, availabilityFilter, tagFilter, dateFrom, dateTo } = useAppStore();
  const activeFilters = [formatFilter, availabilityFilter, tagFilter, dateFrom || dateTo].filter(Boolean).length;
  return (
    <header className="topbar">
      <button className="icon-button topbar__sidebar" onClick={toggleSidebar} aria-label="Toggle sidebar"><PanelLeft size={18} /></button>
      <div className="search-field">
        <Search size={17} aria-hidden="true" />
        <input
          value={search}
          onChange={(event) => setSearch(event.target.value)}
          placeholder="Search models, folders, collections…"
          aria-label="Search library"
        />
        {search && <button onClick={() => setSearch("")} aria-label="Clear search"><X size={15} /></button>}
        <kbd>⌘ K</kbd>
      </div>
      <button className="button button--quiet topbar__import" onClick={onImport}><Link2 size={16} /><span>Import link</span></button>
      <button className={`button button--quiet topbar__filter ${activeFilters ? "is-active" : ""}`} onClick={onFilters}><SlidersHorizontal size={16} /><span>Filter{activeFilters ? ` · ${activeFilters}` : ""}</span></button>
    </header>
  );
}
