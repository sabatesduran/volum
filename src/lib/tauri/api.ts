import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  Collection, DuplicateGroup, DuplicateStats, Folder, LibraryRoot, Material, ModelDetail, ModelQuery, ModelSummary,
  ConfiguredSlicer, Page, PreviewPayload, RelatedModel, ScanStatus, SlicerApplication, SmartCollectionRule, Tag, WebSource, WebSourcePreview
} from "../../types";
import { mockInvoke } from "./mock";

export const isTauri = () => typeof window !== "undefined" && Boolean(window.__TAURI_INTERNALS__);

export async function command<T>(name: string, args: Record<string, unknown> = {}): Promise<T> {
  return isTauri() ? tauriInvoke<T>(name, args) : mockInvoke<T>(name, args);
}

export const api = {
  roots: () => command<LibraryRoot[]>("list_roots"),
  addRoot: (path: string) => command<LibraryRoot>("add_library_root", { path }),
  removeRoot: (rootId: string, keepMetadata = false) => command<void>("remove_library_root", { rootId, keepMetadata }),
  startScan: (rootId: string) => command<void>("start_scan", { rootId }),
  pauseScan: (rootId: string) => command<void>("pause_scan", { rootId }),
  scanStatus: (rootId: string) => command<ScanStatus>("get_scan_status", { rootId }),
  folders: (rootId?: string) => command<Folder[]>("list_folders", { rootId }),
  models: (query: ModelQuery) => command<Page<ModelSummary>>("list_models", { query }),
  model: (modelId: string) => command<ModelDetail>("get_model", { modelId }),
  toggleFavorite: (modelId: string) => command<boolean>("toggle_favorite", { modelId }),
  setFavorite: (modelIds: string[], favorite: boolean) => command<void>("set_favorite", { modelIds, favorite }),
  saveNotes: (modelId: string, notes: string) => command<void>("save_notes", { modelId, notes }),
  collections: () => command<Collection[]>("list_collections"),
  createCollection: (input: { name: string; symbol: string; color: string; smart?: boolean; rule?: SmartCollectionRule }) => command<Collection>("create_collection", { input }),
  updateCollection: (collectionId: string, input: { name: string; symbol: string; color: string; smart?: boolean; rule?: SmartCollectionRule }) => command<Collection>("update_collection", { collectionId, input }),
  deleteCollection: (collectionId: string) => command<void>("delete_collection", { collectionId }),
  addToCollection: (collectionId: string, modelIds: string[]) => command<void>("add_models_to_collection", { collectionId, modelIds }),
  removeFromCollection: (collectionId: string, modelIds: string[]) => command<void>("remove_models_from_collection", { collectionId, modelIds }),
  tags: () => command<Tag[]>("list_tags"),
  saveTag: (input: { id?: string; name: string; color: string }) => command<Tag>("save_tag", { input }),
  deleteTag: (tagId: string) => command<void>("delete_tag", { tagId }),
  setModelTags: (modelId: string, tagIds: string[]) => command<void>("set_model_tags", { modelId, tagIds }),
  addToTag: (tagId: string, modelIds: string[]) => command<void>("add_models_to_tag", { tagId, modelIds }),
  relatedModels: (modelId: string) => command<RelatedModel[]>("list_related_models", { modelId }),
  duplicateStats: () => command<DuplicateStats>("get_duplicate_stats"),
  duplicateGroups: (offset = 0, limit = 48) => command<Page<DuplicateGroup>>("list_duplicate_groups", { offset, limit }),
  materials: () => command<Material[]>("list_materials"),
  saveMaterial: (input: Partial<Material> & Pick<Material, "name" | "materialType" | "spoolPriceMinor" | "currency" | "spoolWeightG">) => command<Material>("save_material", { input }),
  saveEstimate: (input: Record<string, unknown>) => command<void>("save_cost_estimate", { input }),
  calculateCost: (input: { plasticG: number; quantity: number; spoolPriceMinor: number; spoolWeightG: number }) => command<{ costPerPieceMinor: number; batchCostMinor: number }>("calculate_cost", { input }),
  preview: (assetId: string) => command<PreviewPayload>("get_preview_payload", { assetId }),
  plateThumbnail: (assetId: string, plateIndex: number) => command<number[] | null>("get_3mf_plate_thumbnail", { assetId, plateIndex }),
  viewerMesh: (assetId: string, plateIndex?: number) => command<ArrayBuffer>("get_viewer_mesh", { assetId, plateIndex }),
  thumbnail: (assetId: string) => command<number[] | null>("get_cached_thumbnail", { assetId }),
  requestThumbnail: (assetId: string) => command<boolean>("request_thumbnail", { assetId }),
  preferences: () => command<Record<string, string>>("get_preferences"),
  slicers: (customApps: ConfiguredSlicer[] = []) => command<SlicerApplication[]>("list_slicer_apps", { customApps }),
  savePreference: (key: string, value: string) => command<void>("save_preference", { key, value }),
  openAsset: (assetId: string, appPath?: string) => command<void>("open_asset", { assetId, appPath }),
  openExternal: (url: string) => command<void>("open_external_url", { url }),
  previewWebSource: (url: string) => command<WebSourcePreview>("preview_web_source", { url }),
  webSources: () => command<WebSource[]>("list_web_sources"),
  saveWebSource: (preview: WebSourcePreview) => command<WebSource>("save_web_source", { preview }),
  deleteWebSource: (sourceId: string) => command<void>("delete_web_source", { sourceId }),
  attachWebSource: (sourceId: string, localPath: string) => command<WebSource>("attach_web_source_file", { sourceId, localPath }),
  revealAsset: (assetId: string) => command<void>("reveal_asset", { assetId }),
  exportMetadata: (path: string) => command<void>("export_metadata", { path }),
  exportDiagnostics: (path: string, redactPaths = true) => command<void>("export_diagnostics", { path, redactPaths })
};

export async function onBackendEvent<T = unknown>(event: string, handler: (payload: T) => void): Promise<UnlistenFn> {
  if (!isTauri()) return () => undefined;
  return listen<T>(event, ({ payload }) => handler(payload));
}
