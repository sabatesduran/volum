import { create } from "zustand";
import type { Density, ModelSort, Theme, ViewId } from "../types";

interface AppStore {
  view: ViewId;
  selectedFolderId?: string;
  selectedCollectionId?: string;
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
  sidebarCollapsed: boolean;
  selectView: (view: ViewId) => void;
  selectFolder: (id?: string) => void;
  selectCollection: (id?: string) => void;
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
  toggleSidebar: () => void;
}

function persisted<T>(key: string, fallback: T): T {
  const value = localStorage.getItem(key);
  return (value as T | null) ?? fallback;
}

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
  sidebarCollapsed: false,
  selectView: (view) => set({ view, selectedFolderId: undefined, selectedCollectionId: undefined, selectedModelId: undefined, selectedModelIds: [] }),
  selectFolder: (selectedFolderId) => set({ view: "folders", selectedFolderId, selectedCollectionId: undefined, selectedModelId: undefined, selectedModelIds: [] }),
  selectCollection: (selectedCollectionId) =>
    set({ view: "collections", selectedCollectionId, selectedFolderId: undefined, selectedModelId: undefined, selectedModelIds: [] }),
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
  toggleSidebar: () => set((state) => ({ sidebarCollapsed: !state.sidebarCollapsed }))
}));
