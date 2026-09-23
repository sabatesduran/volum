import { useInfiniteQuery, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useMemo, useRef, useState } from "react";
import { ArrowRight, Check, ChevronDown, FolderPlus, Grid2X2, Pencil, Plus, Rows3, ScanSearch, Sparkles, Trash2 } from "lucide-react";
import { open } from "@tauri-apps/plugin-dialog";
import { useAppStore } from "../../app/store";
import { ModelGrid } from "../../components/ModelGrid";
import { DuplicateStacks } from "../../components/DuplicateStacks";
import { SelectionBar } from "../../components/SelectionBar";
import { api, isTauri } from "../../lib/tauri/api";
import type { Density, ModelQuery, ModelSort } from "../../types";
import { plural, t } from "../../lib/i18n";

const sortOptions: Array<[ModelSort, string]> = [
  ["modified", "Recently modified"],
  ["added", "Recently added"],
  ["opened", "Last opened"],
  ["name", "Name A–Z"]
];

function sortLabel(sort: ModelSort) {
  return t(sortOptions.find(([value]) => value === sort)?.[1] ?? "Recently modified");
}

function SortControl({ value, onChange }: { value: ModelSort; onChange: (value: ModelSort) => void }) {
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!open) return;
    const closeOutside = (event: MouseEvent) => { if (!rootRef.current?.contains(event.target as Node)) setOpen(false); };
    const closeEscape = (event: KeyboardEvent) => { if (event.key === "Escape") setOpen(false); };
    document.addEventListener("mousedown", closeOutside);
    window.addEventListener("keydown", closeEscape);
    return () => { document.removeEventListener("mousedown", closeOutside); window.removeEventListener("keydown", closeEscape); };
  }, [open]);
  return <div className={`sort-control ${open ? "is-open" : ""}`} ref={rootRef}><button className="sort-button" onClick={() => setOpen((current) => !current)} aria-haspopup="menu" aria-expanded={open}>{sortLabel(value)}<ChevronDown size={14} /></button>{open && <div className="sort-menu" role="menu">{sortOptions.map(([sort, label]) => <button key={sort} role="menuitemradio" aria-checked={value === sort} onClick={() => { onChange(sort); setOpen(false); }}><span>{t(label)}</span>{value === sort && <Check size={14} />}</button>)}</div>}</div>;
}

function DensityControl() {
  const { density, setDensity } = useAppStore();
  const choices: Array<[Density, React.ReactNode, string]> = [["compact", <Rows3 size={15} />, "Compact"], ["comfortable", <Grid2X2 size={15} />, "Comfortable"], ["large", <ScanSearch size={16} />, "Large preview"]];
  return <div className="segmented density-control" aria-label={t("Grid density")}>{choices.map(([value, icon, label]) => <button key={value} className={density === value ? "is-active" : ""} onClick={() => setDensity(value)} title={t(label)}>{icon}</button>)}</div>;
}

export function LibraryView({ onNewCollection }: { onNewCollection: () => void }) {
  const queryClient = useQueryClient();
  const [addingFolder, setAddingFolder] = useState(false);
  const { search, view, selectedFolderId, selectedCollectionId, selectedSavedSearchId, selectView, selectFolder, selectCollection, formatFilter, availabilityFilter, tagFilter, dateField, dateFrom, dateTo, modelSort, setModelSort } = useAppStore();
  const { data: folders = [] } = useQuery({ queryKey: ["folders"], queryFn: () => api.folders() });
  const { data: collections = [] } = useQuery({ queryKey: ["collections"], queryFn: api.collections });
  const { data: savedSearches = [] } = useQuery({ queryKey: ["saved-searches"], queryFn: api.savedSearches });
  const { data: duplicateStats } = useQuery({ queryKey: ["duplicate-stats"], queryFn: api.duplicateStats, enabled: view === "duplicates" });
  const selectedFolder = folders.find((folder) => folder.id === selectedFolderId);
  const selectedCollection = collections.find((collection) => collection.id === selectedCollectionId);
  const selectedSavedSearch = savedSearches.find((item) => item.id === selectedSavedSearchId);
  const foldersByParent = useMemo(() => {
    const grouped = new Map<string, typeof folders>();
    for (const folder of folders) {
      const parent = folder.parentId ?? "";
      grouped.set(parent, [...(grouped.get(parent) ?? []), folder]);
    }
    return grouped;
  }, [folders]);
  const renderFolders = (parentId = "", depth = 0): React.ReactNode => (foldersByParent.get(parentId) ?? []).map((folder) => {
    const hasChildren = foldersByParent.has(folder.id);
    return <div className="tree-branch" key={folder.id}><button className={`tree-item ${depth > 0 ? "tree-item--child" : ""} ${selectedFolderId === folder.id ? "is-active" : ""}`} style={{ paddingLeft: 9 + depth * 16 }} onClick={() => selectFolder(folder.id)}>{hasChildren ? <ChevronDown size={14} /> : <span />}<span>{folder.name}</span><small>{folder.modelCount}</small></button>{renderFolders(folder.id, depth + 1)}</div>;
  });
  const modelQuery: ModelQuery = {
    search: search || undefined,
    folderId: view === "folders" ? selectedFolderId : undefined,
    collectionId: view === "collections" ? selectedCollectionId : undefined,
    favorite: view === "favorites" || undefined,
    recent: view === "recent" || undefined,
    format: formatFilter || undefined,
    availability: availabilityFilter || undefined,
    tagId: tagFilter || undefined,
    dateField: dateFrom || dateTo ? dateField : undefined,
    dateFrom: dateFrom || undefined,
    dateTo: dateTo || undefined,
    duplicates: view === "duplicates" || undefined,
    sort: modelSort,
    limit: 48
  };
  const { data, isLoading, isFetchingNextPage, hasNextPage, fetchNextPage } = useInfiniteQuery({
    queryKey: ["models", modelQuery],
    enabled: view !== "duplicates",
    initialPageParam: 0,
    queryFn: ({ pageParam }) => api.models({ ...modelQuery, offset: pageParam }),
    getNextPageParam: (page) => page.nextOffset
  });
  const models = data?.pages.flatMap((page) => page.items) ?? [];
  const total = data?.pages[0]?.total ?? 0;
  const pagination = { loadingMore: isFetchingNextPage, hasMore: Boolean(hasNextPage), onLoadMore: () => { void fetchNextPage(); } };
  const duplicateQuery = useInfiniteQuery({
    queryKey: ["duplicate-groups"],
    enabled: view === "duplicates",
    initialPageParam: 0,
    queryFn: ({ pageParam }) => api.duplicateGroups(pageParam, 48),
    getNextPageParam: (page) => page.nextOffset
  });
  const duplicateGroups = duplicateQuery.data?.pages.flatMap((page) => page.items) ?? [];
  const invalidate = () => { queryClient.invalidateQueries({ queryKey: ["models"] }); };
  const addFolder = async () => {
    if (addingFolder) return;
    setAddingFolder(true);
    try {
      const path = isTauri() ? await open({ directory: true, multiple: false, title: t("Choose a 3D project folder") }) : "/Users/you/New Projects";
      if (!path || Array.isArray(path)) return;
      const root = await api.addRoot(path);
      await api.startScan(root.id);
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["roots"] }),
        queryClient.invalidateQueries({ queryKey: ["folders"] }),
        queryClient.invalidateQueries({ queryKey: ["models"] })
      ]);
    } finally {
      setAddingFolder(false);
    }
  };

  if (view === "folders") {
    return (
      <div className="split-browser">
        <aside className="tree-panel">
          <div className="tree-panel__header"><span>{t("Folders")}</span><button className="icon-button icon-button--tiny" onClick={() => void addFolder()} disabled={addingFolder} aria-label={t("Add folder")} title={t("Add folder")}><FolderPlus size={15} /></button></div>
          <button className={`tree-item tree-item--root ${!selectedFolderId ? "is-active" : ""}`} onClick={() => selectFolder()}><ChevronDown size={14} /><span>{t("All folders")}</span><small>{total}</small></button>
          {renderFolders()}
        </aside>
        <section className="content-view content-view--grid">
          <header className="view-header"><div><div className="eyebrow">{t("3D Projects / {folder}", { folder: selectedFolder?.relativePath ?? t("All folders") })}</div><h1>{selectedFolder?.name ?? t("Folders")}</h1><p>{t("{count} projects in this location", { count: total })}</p></div><div className="view-header__controls"><SortControl value={modelSort} onChange={setModelSort} /><DensityControl /></div></header>
          <ModelGrid models={models} loading={isLoading} {...pagination} onChanged={invalidate} /><SelectionBar />
        </section>
      </div>
    );
  }

  if (view === "collections" && !selectedCollectionId) {
    return (
      <section className="content-view collections-view">
        <header className="view-header"><div><div className="eyebrow">{t("Flexible organization")}</div><h1>{t("Collections")}</h1><p>{t("Gather related projects without moving their files.")}</p></div><button className="button button--primary" onClick={onNewCollection}><Plus size={16} /> {t("New collection")}</button></header>
        <div className="collection-cards">{collections.map((collection) => <button key={collection.id} className="collection-card" onClick={() => selectCollection(collection.id)}><div className="collection-card__art" style={{ "--collection-color": collection.color } as React.CSSProperties}><span /><span /><span /><strong>{collection.smart ? <Sparkles size={20} /> : collection.symbol.slice(0, 1).toUpperCase()}</strong></div><div><h2>{collection.name}</h2><p>{collection.smart ? `${t("Smart")} · ` : ""}{plural(collection.modelCount, "{count} project", "{count} projects")}</p></div><ArrowRight size={18} /></button>)}<button className="collection-card collection-card--new" onClick={onNewCollection}><span className="collection-card__plus"><Plus /></span><div><h2>{t("New collection")}</h2><p>{t("Start a new group")}</p></div></button></div>
      </section>
    );
  }


  if (view === "duplicates") {
    return <section className="content-view content-view--grid"><header className="view-header"><div><div className="eyebrow">{t("File and geometry matches")}</div><h1>{t("Duplicates")}</h1><p>{duplicateStats ? `${plural(duplicateStats.groups, "{count} stack", "{count} stacks")} · ${plural(duplicateStats.models, "{count} project", "{count} projects")} · ${plural(duplicateStats.redundantCopies, "{count} redundant copy", "{count} redundant copies")}` : t("Analyzing project geometry…")}</p></div><DensityControl /></header><DuplicateStacks groups={duplicateGroups} loading={duplicateQuery.isLoading} loadingMore={duplicateQuery.isFetchingNextPage} hasMore={Boolean(duplicateQuery.hasNextPage)} onLoadMore={() => { void duplicateQuery.fetchNextPage(); }} /></section>;
  }

  const title = view === "saved" ? selectedSavedSearch?.name ?? t("Saved search") : search ? t("Results for “{search}”", { search }) : view === "favorites" ? t("Favorites") : view === "recent" ? t("Recent") : view === "collections" ? selectedCollection?.name ?? t("Collection") : t("Your library");
  const subtitle = view === "saved" ? t("{count} projects matching this saved search", { count: total }) : search ? t("{count} matching projects", { count: total }) : view === "favorites" ? t("Projects you want close at hand.") : view === "recent" ? t("Projects you added, changed, or opened recently.") : view === "collections" ? selectedCollection?.smart ? t("{count} projects matching this collection’s rules", { count: total }) : t("{count} projects from across your folders", { count: total }) : plural(total, "{count} project in your library", "{count} projects in your library");
  return (
    <section className={`content-view content-view--grid ${view === "library" && !search ? "library-home" : ""}`}>
      <header className="view-header"><div><div className="eyebrow">{view === "library" ? t("Local 3D project library") : t(view === "favorites" ? "Favorites" : view === "recent" ? "Recent" : view === "saved" ? "Saved search" : "Collections")}</div><h1>{title}</h1><p>{subtitle}</p></div><div className="view-header__controls">{view === "saved" && selectedSavedSearch && <><button className="icon-button" title={t("Rename saved search")} aria-label={t("Rename saved search")} onClick={async () => { const name = window.prompt(t("Name this search"), selectedSavedSearch.name); if (!name?.trim()) return; await api.saveSavedSearch({ id: selectedSavedSearch.id, name: name.trim(), query: selectedSavedSearch.query }); await queryClient.invalidateQueries({ queryKey: ["saved-searches"] }); }}><Pencil size={15} /></button><button className="icon-button" title={t("Delete saved search")} aria-label={t("Delete saved search")} onClick={async () => { await api.deleteSavedSearch(selectedSavedSearch.id); await queryClient.invalidateQueries({ queryKey: ["saved-searches"] }); selectView("library"); }}><Trash2 size={16} /></button></>}<SortControl value={modelSort} onChange={setModelSort} /><DensityControl /></div></header>
      {view === "library" && !search && collections.length > 0 && <div className="home-collections"><div className="section-heading"><h2>{t("Collections")}</h2><button onClick={() => useAppStore.getState().selectView("collections")}>{t("View all")} <ArrowRight size={14} /></button></div><div className="home-collection-row">{collections.slice(0, 4).map((collection) => <button key={collection.id} onClick={() => selectCollection(collection.id)}><span style={{ background: collection.color }}>{collection.symbol.slice(0, 1).toUpperCase()}</span><div><strong>{collection.name}</strong><small>{plural(collection.modelCount, "{count} project", "{count} projects")}</small></div></button>)}</div><div className="section-heading section-heading--models"><h2>{sortLabel(modelSort)}</h2><span>{t("{count} total", { count: total })}</span></div></div>}
      <ModelGrid models={models} loading={isLoading} {...pagination} context={view === "recent" ? "date" : search ? "format" : "folder"} onChanged={invalidate} /><SelectionBar />
    </section>
  );
}
