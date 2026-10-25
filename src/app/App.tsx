import { lazy, Suspense, useEffect, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Sidebar } from "../components/Sidebar";
import { Topbar } from "../components/Topbar";
import { Onboarding } from "../components/Onboarding";
import { IndexStatus } from "../components/IndexStatus";
import { LibraryView } from "../features/library/LibraryView";
import { api, onBackendEvent } from "../lib/tauri/api";
import { t } from "../lib/i18n";
import { useAppStore } from "./store";
import { useShallow } from "zustand/react/shallow";

const ModelDetailView = lazy(() => import("../features/model-detail/ModelDetailView").then((module) => ({ default: module.ModelDetailView })));
const SettingsView = lazy(() => import("../features/settings/SettingsView").then((module) => ({ default: module.SettingsView })));
const WebImportsView = lazy(() => import("../features/web-imports/WebImportsView").then((module) => ({ default: module.WebImportsView })));
const FilterPanel = lazy(() => import("../components/FilterPanel").then((module) => ({ default: module.FilterPanel })));
const NewCollectionDialog = lazy(() => import("../components/NewCollectionDialog").then((module) => ({ default: module.NewCollectionDialog })));
const ImportFromWebDialog = lazy(() => import("../components/ImportFromWebDialog").then((module) => ({ default: module.ImportFromWebDialog })));
const ImportProjectPackageDialog = lazy(() => import("../components/ImportProjectPackageDialog").then((module) => ({ default: module.ImportProjectPackageDialog })));
const NewProjectDialog = lazy(() => import("../components/NewProjectDialog").then((module) => ({ default: module.NewProjectDialog })));

export default function App() {
  const queryClient = useQueryClient();
  const { view, selectedModelId, theme, language, density, modelSort, setTheme, setLanguage, setDensity, setModelSort } = useAppStore(useShallow((state) => ({ view: state.view, selectedModelId: state.selectedModelId, theme: state.theme, language: state.language, density: state.density, modelSort: state.modelSort, setTheme: state.setTheme, setLanguage: state.setLanguage, setDensity: state.setDensity, setModelSort: state.setModelSort })));
  const [newCollection, setNewCollection] = useState(false);
  const [filtersOpen, setFiltersOpen] = useState(false);
  const [webImportOpen, setWebImportOpen] = useState(false);
  const [projectImportOpen, setProjectImportOpen] = useState(false);
  const [newProjectOpen, setNewProjectOpen] = useState(false);
  const { data: roots, isLoading } = useQuery({ queryKey: ["roots"], queryFn: api.roots });
  const { data: preferences } = useQuery({ queryKey: ["preferences"], queryFn: api.preferences });
  const [preferencesHydrated, setPreferencesHydrated] = useState(false);

  useEffect(() => {
    if (!preferences || preferencesHydrated) return;
    if (["system", "light", "dark"].includes(preferences.ui_theme)) setTheme(preferences.ui_theme as typeof theme);
    if (["system", "en", "ca", "es"].includes(preferences.ui_language)) setLanguage(preferences.ui_language as typeof language);
    if (["compact", "comfortable", "large"].includes(preferences.ui_density)) setDensity(preferences.ui_density as typeof density);
    if (["name", "added", "modified", "opened"].includes(preferences.ui_model_sort)) setModelSort(preferences.ui_model_sort as typeof modelSort);
    setPreferencesHydrated(true);
  }, [density, language, modelSort, preferences, preferencesHydrated, setDensity, setLanguage, setModelSort, setTheme, theme]);

  useEffect(() => {
    if (!preferencesHydrated) return;
    const next = { ui_theme: theme, ui_language: language, ui_density: density, ui_model_sort: modelSort };
    const changed = Object.entries(next).filter(([key, value]) => preferences?.[key] !== value);
    if (!changed.length) return;
    void Promise.all(changed.map(([key, value]) => api.savePreference(key, value))).then(() => {
      queryClient.setQueryData<Record<string, string>>(["preferences"], (current = {}) => ({ ...current, ...next }));
    });
  }, [density, language, modelSort, preferences, preferencesHydrated, queryClient, theme]);

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
      if ((event.metaKey || event.ctrlKey) && event.key.toLocaleLowerCase() === "n") {
        event.preventDefault();
        if (!selectedModelId) setNewProjectOpen(true);
      }
      if (event.key === "Escape" && selectedModelId) useAppStore.getState().selectModel();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [selectedModelId]);

  useEffect(() => {
    const unlisteners: Array<() => void> = [];
    let disposed = false;
    let modelsTimer: number | undefined;
    const invalidateModels = () => {
      if (modelsTimer != null) return;
      modelsTimer = window.setTimeout(() => {
        modelsTimer = undefined;
        ["models", "folders", "library-assets", "collections", "duplicate-groups", "duplicate-stats"]
          .forEach((key) => queryClient.invalidateQueries({ queryKey: [key] }));
        if (selectedModelId) {
          void queryClient.invalidateQueries({ queryKey: ["model", selectedModelId], exact: true });
          void queryClient.invalidateQueries({ queryKey: ["related-models", selectedModelId], exact: true });
        }
      }, 200);
    };
    Promise.all([
      onBackendEvent("models-changed", invalidateModels),
      onBackendEvent("root-status-changed", () => queryClient.invalidateQueries({ queryKey: ["roots"] })),
      onBackendEvent("web-sources-changed", () => queryClient.invalidateQueries({ queryKey: ["web-sources"] })),
      onBackendEvent("backups-changed", () => { queryClient.invalidateQueries({ queryKey: ["backup-destinations"] }); queryClient.invalidateQueries({ queryKey: ["backup-runs"] }); queryClient.invalidateQueries({ queryKey: ["backup-archives"] }); })
    ]).then((values) => disposed ? values.forEach((unlisten) => unlisten()) : unlisteners.push(...values));
    return () => {
      disposed = true;
      if (modelsTimer != null) window.clearTimeout(modelsTimer);
      unlisteners.forEach((unlisten) => unlisten());
    };
  }, [queryClient, selectedModelId]);

  if (isLoading) return <div className="app-boot"><div className="app-boot__mark">V</div></div>;
  if (!roots?.length) return <Onboarding onComplete={() => queryClient.invalidateQueries({ queryKey: ["roots"] })} />;

  return (
    <div className="app-shell" data-language={language}>
      {!selectedModelId && <Sidebar onNewCollection={() => setNewCollection(true)} />}
      <main className={`app-main ${selectedModelId ? "app-main--detail" : ""}`}>
        {!selectedModelId && view !== "settings" && view !== "web" && <Topbar onNewProject={() => setNewProjectOpen(true)} onImport={() => setWebImportOpen(true)} onProjectImport={() => setProjectImportOpen(true)} onFilters={() => setFiltersOpen((value) => !value)} />}
        <Suspense fallback={<div className="detail-loading"><span>{t("Preparing project…")}</span></div>}>{selectedModelId ? <ModelDetailView modelId={selectedModelId} /> : view === "settings" ? <SettingsView /> : view === "web" ? <WebImportsView onImport={() => setWebImportOpen(true)} /> : <LibraryView roots={roots} onNewCollection={() => setNewCollection(true)} onNewProject={() => setNewProjectOpen(true)} />}</Suspense>
      </main>
      {!selectedModelId && <IndexStatus roots={roots} />}
      <Suspense fallback={null}>
        {filtersOpen && !selectedModelId && <FilterPanel onClose={() => setFiltersOpen(false)} />}
        {newCollection && <NewCollectionDialog onClose={() => setNewCollection(false)} />}
        {webImportOpen && <ImportFromWebDialog onClose={() => setWebImportOpen(false)} />}
        {projectImportOpen && <ImportProjectPackageDialog onClose={() => setProjectImportOpen(false)} />}
        {newProjectOpen && <NewProjectDialog projectIds={useAppStore.getState().selectedModelIds} onClose={() => setNewProjectOpen(false)} />}
      </Suspense>
    </div>
  );
}
