import { lazy, Suspense, useEffect, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Sidebar } from "../components/Sidebar";
import { Topbar } from "../components/Topbar";
import { NewCollectionDialog } from "../components/NewCollectionDialog";
import { Onboarding } from "../components/Onboarding";
import { IndexStatus } from "../components/IndexStatus";
import { FilterPanel } from "../components/FilterPanel";
import { LibraryView } from "../features/library/LibraryView";
import { SettingsView } from "../features/settings/SettingsView";
import { WebImportsView } from "../features/web-imports/WebImportsView";
import { ImportFromWebDialog } from "../components/ImportFromWebDialog";
import { api, onBackendEvent } from "../lib/tauri/api";
import { useAppStore } from "./store";

const ModelDetailView = lazy(() => import("../features/model-detail/ModelDetailView").then((module) => ({ default: module.ModelDetailView })));

export default function App() {
  const queryClient = useQueryClient();
  const { view, selectedModelId, theme, setSearch } = useAppStore();
  const [newCollection, setNewCollection] = useState(false);
  const [filtersOpen, setFiltersOpen] = useState(false);
  const [webImportOpen, setWebImportOpen] = useState(false);
  const { data: roots, isLoading } = useQuery({ queryKey: ["roots"], queryFn: api.roots });

  useEffect(() => {
    const apply = () => {
      const dark = theme === "dark" || (theme === "system" && window.matchMedia("(prefers-color-scheme: dark)").matches);
      document.documentElement.dataset.theme = dark ? "dark" : "light";
    };
    apply();
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    media.addEventListener("change", apply);
    return () => media.removeEventListener("change", apply);
  }, [theme]);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLocaleLowerCase() === "k") {
        event.preventDefault();
        document.querySelector<HTMLInputElement>(".search-field input")?.focus();
      }
      if (event.key === "Escape" && selectedModelId) useAppStore.getState().selectModel();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [selectedModelId]);

  useEffect(() => {
    const unlisteners: Array<() => void> = [];
    Promise.all([
      onBackendEvent("models-changed", () => { queryClient.invalidateQueries({ queryKey: ["models"] }); queryClient.invalidateQueries({ queryKey: ["folders"] }); queryClient.invalidateQueries({ queryKey: ["collections"] }); queryClient.invalidateQueries({ queryKey: ["related-models"] }); queryClient.invalidateQueries({ queryKey: ["duplicate-groups"] }); queryClient.invalidateQueries({ queryKey: ["duplicate-stats"] }); queryClient.invalidateQueries({ queryKey: ["web-sources"] }); }),
      onBackendEvent("root-status-changed", () => queryClient.invalidateQueries({ queryKey: ["roots"] })),
      onBackendEvent("web-sources-changed", () => queryClient.invalidateQueries({ queryKey: ["web-sources"] }))
    ]).then((values) => unlisteners.push(...values));
    return () => unlisteners.forEach((unlisten) => unlisten());
  }, [queryClient]);

  if (isLoading) return <div className="app-boot"><div className="app-boot__mark">V</div></div>;
  if (!roots?.length) return <Onboarding onComplete={() => queryClient.invalidateQueries({ queryKey: ["roots"] })} />;

  return (
    <div className="app-shell">
      {!selectedModelId && <Sidebar onNewCollection={() => setNewCollection(true)} />}
      <main className={`app-main ${selectedModelId ? "app-main--detail" : ""}`}>
        {!selectedModelId && view !== "settings" && view !== "web" && <Topbar onImport={() => setWebImportOpen(true)} onFilters={() => setFiltersOpen((value) => !value)} />}
        {selectedModelId ? <Suspense fallback={<div className="detail-loading"><span>Preparing viewer…</span></div>}><ModelDetailView modelId={selectedModelId} /></Suspense> : view === "settings" ? <SettingsView /> : view === "web" ? <WebImportsView onImport={() => setWebImportOpen(true)} /> : <LibraryView onNewCollection={() => setNewCollection(true)} />}
      </main>
      {!selectedModelId && <IndexStatus roots={roots} />}
      {filtersOpen && !selectedModelId && <FilterPanel onClose={() => setFiltersOpen(false)} />}
      {newCollection && <NewCollectionDialog onClose={() => setNewCollection(false)} />}
      {webImportOpen && <ImportFromWebDialog onClose={() => setWebImportOpen(false)} />}
    </div>
  );
}
