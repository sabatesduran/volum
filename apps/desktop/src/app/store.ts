import { create } from "zustand";
import type { Density, LanguagePreference, ModelQuery, ModelSort, Theme, ViewId } from "../types";
import { resolveLanguagePreference, setAppLanguage } from "../lib/i18n";

interface AppStore {
  view: ViewId;
  selectedFolderId?: string;
  selectedCollectionId?: string;
  selectedSavedSearchId?: string;
  selectedModelId?: string;
  selectedModelIds: string[];
  search: string;
  formatFilter: string;
  availabilityFilter: "" | "available" | "offline";
  tagFilter: string;
  dateField: "added" | "modified";
  dateFrom: string;
  dateTo: string;
  modelSort: ModelSort;
  density: Density;
  theme: Theme;
  language: LanguagePreference;
  sidebarCollapsed: boolean;
  selectView: (view: ViewId) => void;
  selectFolder: (id?: string) => void;
  selectCollection: (id?: string) => void;
  selectSavedSearch: (id: string, query: ModelQuery) => void;
  selectModel: (id?: string) => void;
  toggleModelSelection: (id: string) => void;
  clearModelSelection: () => void;
  setSearch: (value: string) => void;
  setFormatFilter: (value: string) => void;
  setAvailabilityFilter: (value: "" | "available" | "offline") => void;
  setTagFilter: (value: string) => void;
  setDateField: (value: "added" | "modified") => void;
  setDateRange: (from: string, to: string) => void;
  setModelSort: (sort: ModelSort) => void;
  clearFilters: () => void;
  setDensity: (density: Density) => void;
  setTheme: (theme: Theme) => void;
  setLanguage: (language: LanguagePreference) => void;
  toggleSidebar: () => void;
}

function persisted<T>(key: string, fallback: T): T {
  const value = localStorage.getItem(key);
  return (value as T | null) ?? fallback;
}

function persistedLanguage(): LanguagePreference {
  const value = localStorage.getItem("volum:language");
  return value === "en" || value === "ca" || value === "es" || value === "system" ? value : "system";
}

const initialLanguage = persistedLanguage();
setAppLanguage(resolveLanguagePreference(initialLanguage));

export const useAppStore = create<AppStore>((set) => ({
  view: "library",
  search: "",
  selectedModelIds: [],
  formatFilter: "",
  availabilityFilter: "",
  tagFilter: "",
  dateField: "modified",
  dateFrom: "",
  dateTo: "",
  modelSort: persisted<ModelSort>("volum:model-sort", "modified"),
  density: persisted<Density>("volum:density", "comfortable"),
  theme: persisted<Theme>("volum:theme", "system"),
  language: initialLanguage,
  sidebarCollapsed: false,
  selectView: (view) => set({ view, selectedFolderId: undefined, selectedCollectionId: undefined, selectedSavedSearchId: undefined, selectedModelId: undefined, selectedModelIds: [] }),
  selectFolder: (selectedFolderId) => set({ view: "folders", selectedFolderId, selectedCollectionId: undefined, selectedSavedSearchId: undefined, selectedModelId: undefined, selectedModelIds: [] }),
  selectCollection: (selectedCollectionId) =>
    set({ view: "collections", selectedCollectionId, selectedFolderId: undefined, selectedSavedSearchId: undefined, selectedModelId: undefined, selectedModelIds: [] }),
  selectSavedSearch: (selectedSavedSearchId, query) => set({
    view: "saved",
    selectedSavedSearchId,
    selectedFolderId: undefined,
    selectedCollectionId: undefined,
    selectedModelId: undefined,
    selectedModelIds: [],
    search: query.search ?? "",
    formatFilter: query.format ?? "",
    availabilityFilter: query.availability ?? "",
    tagFilter: query.tagId ?? "",
    dateField: query.dateField ?? "modified",
    dateFrom: query.dateFrom ?? "",
    dateTo: query.dateTo ?? "",
    modelSort: query.sort ?? "modified"
  }),
  selectModel: (selectedModelId) => set({ selectedModelId, selectedModelIds: [] }),
  toggleModelSelection: (id) => set((state) => ({ selectedModelIds: state.selectedModelIds.includes(id) ? state.selectedModelIds.filter((value) => value !== id) : [...state.selectedModelIds, id] })),
  clearModelSelection: () => set({ selectedModelIds: [] }),
  setSearch: (search) => set({ search }),
  setFormatFilter: (formatFilter) => set({ formatFilter }),
  setAvailabilityFilter: (availabilityFilter) => set({ availabilityFilter }),
  setTagFilter: (tagFilter) => set({ tagFilter }),
  setDateField: (dateField) => set({ dateField }),
  setDateRange: (dateFrom, dateTo) => set({ dateFrom, dateTo }),
  setModelSort: (modelSort) => {
    localStorage.setItem("volum:model-sort", modelSort);
    set({ modelSort });
  },
  clearFilters: () => set({ formatFilter: "", availabilityFilter: "", tagFilter: "", dateField: "modified", dateFrom: "", dateTo: "" }),
  setDensity: (density) => {
    localStorage.setItem("volum:density", density);
    set({ density });
  },
  setTheme: (theme) => {
    localStorage.setItem("volum:theme", theme);
    set({ theme });
  },
  setLanguage: (language) => {
    localStorage.setItem("volum:language", language);
    setAppLanguage(resolveLanguagePreference(language));
    set({ language });
  },
  toggleSidebar: () => set((state) => ({ sidebarCollapsed: !state.sidebarCollapsed }))
}));
