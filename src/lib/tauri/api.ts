import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  AssemblyStep, BackupArchive, BackupDestination, BackupDestinationInput, BackupRun, Collection, CustomFieldDefinition, DuplicateGroup, DuplicateStats, Folder, LibraryRoot, Material, ModelDetail, ModelQuery, ModelSummary,
  BackendConnectionStatus, CompanionPairingInfo, CompanionStatus, ConfiguredSlicer, ConsolidationPreview, InboxImportResult, LibraryAssetCandidate, ManagedLibraryStatus, MirroredAsset, MirrorSyncReport, Page, PreparedRestore, PreviewPayload, ProjectBomItem, ProjectDocumentation, ProjectPackagePreview, ProjectPhoto, RelatedModel, SavedSearch, ScanStatus, SlicerApplication, SmartCollectionRule, Tag, WebSource, WebSourcePreview
} from "../../types";
import { mockInvoke } from "./mock";

export const isTauri = () => typeof window !== "undefined" && Boolean(window.__TAURI_INTERNALS__);

export async function command<T>(name: string, args: Record<string, unknown> = {}): Promise<T> {
  return isTauri() ? tauriInvoke<T>(name, args) : mockInvoke<T>(name, args);
}

let activeBackend: "local" | "remote" = "local";
let connectionStatus: BackendConnectionStatus | undefined;

export async function initializeBackend(): Promise<BackendConnectionStatus> {
  const status = await command<BackendConnectionStatus>("get_backend_connection");
  activeBackend = status.mode;
  connectionStatus = status;
  return status;
}

export function backendMode() { return activeBackend; }
export function currentBackendConnection() { return connectionStatus; }

export async function connectRemote(pairingPayload: string): Promise<BackendConnectionStatus> {
  const status = await command<BackendConnectionStatus>("connect_remote_server", { pairingPayload });
  activeBackend = status.mode;
  connectionStatus = status;
  return status;
}

export async function useLocalBackend(): Promise<BackendConnectionStatus> {
  const status = await command<BackendConnectionStatus>("use_local_backend");
  activeBackend = status.mode;
  connectionStatus = status;
  return status;
}

export async function useRemoteBackend(): Promise<BackendConnectionStatus> {
  const status = await command<BackendConnectionStatus>("use_remote_backend");
  activeBackend = status.mode;
  connectionStatus = status;
  return status;
}

export async function forgetRemoteServer(): Promise<BackendConnectionStatus> {
  const status = await command<BackendConnectionStatus>("forget_remote_server");
  activeBackend = status.mode;
  connectionStatus = status;
  return status;
}

export async function configureRemoteMirror(path: string | undefined, enabled: boolean): Promise<BackendConnectionStatus> {
  const status = await command<BackendConnectionStatus>("configure_remote_mirror", { path, enabled });
  connectionStatus = status;
  return status;
}

export function syncRemoteMirror(): Promise<MirrorSyncReport> {
  return command<MirrorSyncReport>("sync_remote_mirror");
}

export function listMirroredAssets(): Promise<MirroredAsset[]> {
  return command<MirroredAsset[]>("list_mirrored_assets");
}

export function remoteServerStatus(): Promise<{ serverId: string; libraryRevision: string; projectCount: number }> {
  return remoteJson("/api/v1/status");
}

type RemoteMethod = "GET" | "POST" | "PUT" | "PATCH" | "DELETE";
function remoteJson<T>(path: string, method: RemoteMethod = "GET", body?: unknown): Promise<T> {
  return command<T>("remote_json_request", { request: { path, method, body } });
}
function remoteBinary(path: string): Promise<ArrayBuffer> {
  return command<ArrayBuffer>("remote_binary_request", { path });
}
function remoteUpload(filePath: string, destination: { projectId: string } | { projectName: string }) {
  const filename = filePath.split(/[\\/]/).pop() ?? "";
  return command<{ projectId: string; assetId: string; filename: string; byteSize: number }>("remote_upload_file", { request: { filePath, filename, ...destination } });
}
const remotePhotoProjects = new Map<string, string>();
const remoteBomProjects = new Map<string, string>();
const remoteStepProjects = new Map<string, string>();
function rememberRemotePhotos(projectId: string, documentation: ProjectDocumentation) {
  documentation.photos.forEach((photo) => remotePhotoProjects.set(photo.id, projectId));
  documentation.bomItems.forEach((item) => remoteBomProjects.set(item.id, projectId));
  documentation.assemblySteps.forEach((step) => remoteStepProjects.set(step.id, projectId));
  return documentation;
}
function queryString(query: object) {
  const params = new URLSearchParams();
  Object.entries(query as Record<string, unknown>).forEach(([key, value]) => {
    if (value !== undefined && value !== null && value !== "") params.set(key, String(value));
  });
  const value = params.toString();
  return value ? `?${value}` : "";
}
function remoteUnavailable(name: string): Promise<never> {
  return Promise.reject(new Error(`${name} is available only when this Desktop uses its local library.`));
}
async function remoteProject(modelId: string): Promise<ModelDetail & { revision: string }> {
  const model = await remoteJson<ModelDetail & { revision: string }>(`/api/v1/projects/${encodeURIComponent(modelId)}`);
  return {
    ...model,
    rootPath: "Remote server",
    assets: model.assets.map((asset) => ({ ...asset, relativePath: asset.filename }))
  };
}
async function patchRemoteProject(modelId: string, change: Record<string, unknown>) {
  const model = await remoteProject(modelId);
  return remoteJson<ModelDetail & { revision: string }>(`/api/v1/projects/${encodeURIComponent(modelId)}`, "PATCH", { ...change, ifUnmodifiedSince: model.revision });
}

const localApi = {
  roots: () => command<LibraryRoot[]>("list_roots"),
  addRoot: (path: string) => command<LibraryRoot>("add_library_root", { path }),
  removeRoot: (rootId: string, keepMetadata = false) => command<void>("remove_library_root", { rootId, keepMetadata }),
  reconnectRoot: (rootId: string, path: string) => command<LibraryRoot>("reconnect_library_root", { rootId, path }),
  startScan: (rootId: string) => command<void>("start_scan", { rootId }),
  pauseScan: (rootId: string) => command<void>("pause_scan", { rootId }),
  scanStatus: (rootId: string) => command<ScanStatus>("get_scan_status", { rootId }),
  managedLibrary: () => command<ManagedLibraryStatus>("get_managed_library_status"),
  configureManagedLibrary: (path: string) => command<LibraryRoot>("configure_managed_library", { path }),
  libraryAssets: () => command<LibraryAssetCandidate[]>("list_library_assets"),
  importInbox: () => command<InboxImportResult>("import_managed_inbox"),
  createProject: (input: { name: string; filePaths: string[]; assetIds: string[]; transferMode: "move" | "copy" | "link" }) => command<string>("create_managed_project", { input }),
  addProjectFiles: (input: { projectId: string; filePaths: string[]; assetIds: string[]; transferMode: "move" | "copy" | "link" }) => command<string[]>("add_files_to_project", { input }),
  renameProject: (projectId: string, name: string) => command<void>("rename_project", { projectId, name }),
  renameProjectFile: (assetId: string, name: string) => command<void>("rename_project_file", { assetId, name }),
  consolidationPreview: (projectId: string) => command<ConsolidationPreview>("preview_project_consolidation", { projectId }),
  convertProjectToManaged: (projectId: string, transferMode: "move" | "copy") => command<void>("convert_project_to_managed", { input: { projectId, transferMode } }),
  trashProject: (projectId: string) => command<void>("trash_project", { projectId }),
  restoreProject: (projectId: string) => command<void>("restore_project_from_trash", { projectId }),
  deleteProjectPermanently: (projectId: string) => command<void>("delete_project_permanently", { projectId }),
  folders: (rootId?: string) => command<Folder[]>("list_folders", { rootId }),
  models: (query: ModelQuery) => command<Page<ModelSummary>>("list_models", { query }),
  model: (modelId: string) => command<ModelDetail>("get_model", { modelId }),
  toggleFavorite: (modelId: string) => command<boolean>("toggle_favorite", { modelId }),
  setFavorite: (modelIds: string[], favorite: boolean) => command<void>("set_favorite", { modelIds, favorite }),
  saveNotes: (modelId: string, notes: string) => command<void>("save_notes", { modelId, notes }),
  projectDocumentation: (modelId: string) => command<ProjectDocumentation>("get_project_documentation", { modelId }),
  addProjectPhoto: (modelId: string, path: string) => command<ProjectPhoto>("add_project_photo", { modelId, path }),
  updateProjectPhoto: (input: { id: string; caption: string; sortOrder: number; isCover: boolean }) => command<ProjectPhoto>("update_project_photo", { input }),
  deleteProjectPhoto: (photoId: string) => command<void>("delete_project_photo", { photoId }),
  projectPhoto: (photoId: string, thumbnail = true) => command<ArrayBuffer>("get_project_photo", { photoId, thumbnail }),
  customFields: () => command<CustomFieldDefinition[]>("list_custom_field_definitions"),
  saveCustomField: (input: { id?: string; name: string; fieldType: CustomFieldDefinition["fieldType"]; unit?: string; options: string[] }) => command<CustomFieldDefinition>("save_custom_field_definition", { input }),
  deleteCustomField: (fieldId: string) => command<void>("delete_custom_field_definition", { fieldId }),
  setProjectCustomValue: (modelId: string, fieldId: string, value?: string | number | boolean) => command<void>("set_project_custom_value", { input: { modelId, fieldId, value } }),
  saveBomItem: (input: { id?: string; modelId: string; kind: "printed" | "purchased"; linkedAssetId?: string; name: string; quantity: number; unit: string; url?: string; notes: string; sortOrder?: number }) => command<ProjectBomItem>("save_project_bom_item", { input }),
  deleteBomItem: (itemId: string) => command<void>("delete_project_bom_item", { itemId }),
  saveAssemblyStep: (input: { id?: string; modelId: string; title: string; instructions: string; sortOrder?: number; bomItemIds: string[]; photoIds: string[] }) => command<AssemblyStep>("save_assembly_step", { input }),
  deleteAssemblyStep: (stepId: string) => command<void>("delete_assembly_step", { stepId }),
  inspectProjectPackage: (path: string) => command<ProjectPackagePreview>("inspect_project_package", { path }),
  exportProjectPackage: (modelId: string, path: string) => command<void>("export_project_package", { modelId, path }),
  importProjectPackage: (input: { path: string; rootId: string; relativeFolder: string }) => command<string>("import_project_package", { input }),
  savedSearches: () => command<SavedSearch[]>("list_saved_searches"),
  saveSavedSearch: (input: { id?: string; name: string; query: ModelQuery }) => command<SavedSearch>("save_saved_search", { input }),
  deleteSavedSearch: (searchId: string) => command<void>("delete_saved_search", { searchId }),
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
  mergeProjects: (keeperId: string, projectIds: string[]) => command<void>("merge_projects", { input: { keeperId, projectIds } }),
  splitProject: (projectId: string, assetIds: string[], name: string) => command<string>("split_project", { input: { projectId, assetIds, name } }),
  setProjectPrimary: (projectId: string, assetId: string) => command<void>("set_project_primary_asset", { projectId, assetId }),
  dismissDuplicate: (matchKey: string) => command<void>("dismiss_duplicate_match", { matchKey }),
  cleanupDuplicate: (input: { matchKey: string; keeperId: string; duplicateIds: string[]; moveToTrash: boolean }) => command<void>("cleanup_duplicate_group", { input }),
  materials: () => command<Material[]>("list_materials"),
  saveMaterial: (input: Partial<Material> & Pick<Material, "name" | "materialType" | "spoolPriceMinor" | "currency" | "spoolWeightG">) => command<Material>("save_material", { input }),
  saveEstimate: (input: Record<string, unknown>) => command<void>("save_cost_estimate", { input }),
  calculateCost: (input: { plasticG: number; quantity: number; spoolPriceMinor: number; spoolWeightG: number }) => command<{ costPerPieceMinor: number; batchCostMinor: number }>("calculate_cost", { input }),
  preview: (assetId: string) => command<PreviewPayload>("get_preview_payload", { assetId }),
  plateThumbnail: (assetId: string, plateIndex: number) => command<ArrayBuffer>("get_3mf_plate_thumbnail", { assetId, plateIndex }),
  viewerMesh: (assetId: string, plateIndex?: number) => command<ArrayBuffer>("get_viewer_mesh", { assetId, plateIndex }),
  thumbnail: (assetId: string) => command<ArrayBuffer>("get_cached_thumbnail", { assetId }),
  requestThumbnail: (assetId: string) => command<boolean>("request_thumbnail", { assetId }),
  cancelThumbnail: (assetId: string) => command<void>("cancel_thumbnail_request", { assetId }),
  clearPreviewCache: () => command<void>("clear_preview_cache"),
  preferences: () => command<Record<string, string>>("get_preferences"),
  slicers: (customApps: ConfiguredSlicer[] = []) => command<SlicerApplication[]>("list_slicer_apps", { customApps }),
  savePreference: (key: string, value: string) => command<void>("save_preference", { key, value }),
  setMenuBarEnabled: (enabled: boolean) => command<boolean>("set_menu_bar_enabled", { enabled }),
  openAsset: (assetId: string, appPath?: string) => command<void>("open_asset", { assetId, appPath }),
  openExternal: (url: string) => command<void>("open_external_url", { url }),
  previewWebSource: (url: string) => command<WebSourcePreview>("preview_web_source", { url }),
  webSources: () => command<WebSource[]>("list_web_sources"),
  saveWebSource: (preview: WebSourcePreview) => command<WebSource>("save_web_source", { preview }),
  deleteWebSource: (sourceId: string) => command<void>("delete_web_source", { sourceId }),
  attachWebSource: (sourceId: string, localPath: string) => command<WebSource>("attach_web_source_file", { sourceId, localPath }),
  revealAsset: (assetId: string) => command<void>("reveal_asset", { assetId }),
  exportMetadata: (path: string) => command<void>("export_metadata", { path }),
  exportDiagnostics: (path: string, redactPaths = true) => command<void>("export_diagnostics", { path, redactPaths }),
  backupDestinations: () => command<BackupDestination[]>("list_backup_destinations"),
  saveBackupDestination: (input: BackupDestinationInput) => command<BackupDestination>("save_backup_destination", { input }),
  deleteBackupDestination: (destinationId: string) => command<void>("delete_backup_destination", { destinationId }),
  testBackupDestination: (destinationId: string) => command<void>("test_backup_destination", { destinationId }),
  runBackup: (destinationId?: string) => command<BackupRun[]>("run_backup", { destinationId }),
  backupRuns: (limit = 20) => command<BackupRun[]>("list_backup_runs", { limit }),
  backupArchives: (destinationId: string) => command<BackupArchive[]>("list_destination_archives", { destinationId }),
  prepareRestoreFromPath: (path: string) => command<PreparedRestore>("prepare_restore_from_path", { path }),
  prepareRestoreFromDestination: (destinationId: string, key: string) => command<PreparedRestore>("prepare_restore_from_destination", { destinationId, key }),
  commitPreparedRestore: (token: string) => command<void>("commit_prepared_restore", { token }),
  cancelPreparedRestore: (token: string) => command<void>("cancel_prepared_restore", { token }),
  companionStatus: () => command<CompanionStatus>("get_companion_status"),
  setCompanionEnabled: (enabled: boolean) => command<CompanionStatus>("set_companion_enabled", { enabled }),
  setCompanionOrganizationWrites: (enabled: boolean) => command<CompanionStatus>("set_companion_organization_writes", { enabled }),
  createCompanionPairing: () => command<CompanionPairingInfo>("create_companion_pairing"),
  revokeCompanionDevice: (deviceId: string) => command<CompanionStatus>("revoke_companion_device", { deviceId })
};

const remoteApi: Partial<Record<keyof typeof localApi, (...args: any[]) => any>> = {
  roots: async () => {
    const roots = await remoteJson<Array<Omit<LibraryRoot, "path">>>("/api/v1/roots");
    return roots.map((root) => ({ ...root, path: connectionStatus?.serverName ?? "Remote server" }));
  },
  folders: (rootId?: string) => remoteJson<Folder[]>(`/api/v1/folders${queryString({ rootId })}`),
  models: (query: ModelQuery) => remoteJson<Page<ModelSummary>>(`/api/v1/projects${queryString(query)}`),
  model: remoteProject,
  toggleFavorite: async (modelId: string) => {
    const model = await remoteProject(modelId);
    const updated = await remoteJson<ModelDetail & { revision: string }>(`/api/v1/projects/${encodeURIComponent(modelId)}`, "PATCH", { favorite: !model.favorite, ifUnmodifiedSince: model.revision });
    return updated.favorite;
  },
  setFavorite: async (modelIds: string[], favorite: boolean) => { for (const id of modelIds) await patchRemoteProject(id, { favorite }); },
  saveNotes: async (modelId: string, notes: string) => { await patchRemoteProject(modelId, { notes }); },
  setModelTags: async (modelId: string, tagIds: string[]) => { await patchRemoteProject(modelId, { tagIds }); },
  renameProject: async (projectId: string, name: string) => { await patchRemoteProject(projectId, { displayName: name }); },
  trashProject: async (projectId: string) => { await remoteJson(`/api/v1/projects/${encodeURIComponent(projectId)}/trash`, "PUT"); },
  restoreProject: async (projectId: string) => { await remoteJson(`/api/v1/projects/${encodeURIComponent(projectId)}/restore`, "POST"); },
  deleteProjectPermanently: async (projectId: string) => { await remoteJson(`/api/v1/projects/${encodeURIComponent(projectId)}/trash`, "DELETE"); },
  projectDocumentation: async (modelId: string) => {
    const result = await remoteJson<ProjectDocumentation & { revision: string }>(`/api/v1/projects/${encodeURIComponent(modelId)}/documentation`);
    return rememberRemotePhotos(modelId, result);
  },
  addProjectPhoto: async (modelId: string, path: string) => {
    const model = await remoteProject(modelId);
    const filename = path.split(/[\\/]/).pop() ?? "";
    const documentation = await command<ProjectDocumentation & { revision: string }>("remote_upload_photo", { request: { filePath: path, filename, projectId: modelId, ifUnmodifiedSince: model.revision } });
    rememberRemotePhotos(modelId, documentation);
    const added = documentation.photos.find((photo) => photo.originalFilename === filename) ?? documentation.photos.at(-1);
    if (!added) throw new Error("The server did not return the uploaded photo.");
    return added;
  },
  updateProjectPhoto: async (input: { id: string; caption: string; sortOrder: number; isCover: boolean }) => {
    const modelId = remotePhotoProjects.get(input.id);
    if (!modelId) throw new Error("Refresh this project's documentation before updating its photo.");
    const model = await remoteProject(modelId);
    const documentation = await remoteJson<ProjectDocumentation & { revision: string }>(`/api/v1/projects/${encodeURIComponent(modelId)}/photos/${encodeURIComponent(input.id)}`, "PATCH", { caption: input.caption, sortOrder: input.sortOrder, isCover: input.isCover, ifUnmodifiedSince: model.revision });
    rememberRemotePhotos(modelId, documentation);
    const updated = documentation.photos.find((photo) => photo.id === input.id);
    if (!updated) throw new Error("The server did not return the updated photo.");
    return updated;
  },
  deleteProjectPhoto: async (photoId: string) => {
    const modelId = remotePhotoProjects.get(photoId);
    if (!modelId) throw new Error("Refresh this project's documentation before deleting its photo.");
    const model = await remoteProject(modelId);
    await remoteJson(`/api/v1/projects/${encodeURIComponent(modelId)}/photos/${encodeURIComponent(photoId)}`, "DELETE", { ifUnmodifiedSince: model.revision });
    remotePhotoProjects.delete(photoId);
  },
  projectPhoto: (photoId: string, thumbnail = true) => remoteBinary(`/api/v1/photos/${encodeURIComponent(photoId)}${queryString({ thumbnail })}`),
  customFields: () => remoteJson<CustomFieldDefinition[]>("/api/v1/custom-fields"),
  saveCustomField: (input: { id?: string; name: string; fieldType: CustomFieldDefinition["fieldType"]; unit?: string; options: string[] }) => remoteJson<CustomFieldDefinition>("/api/v1/custom-fields", "POST", input),
  deleteCustomField: async (fieldId: string) => { await remoteJson(`/api/v1/custom-fields/${encodeURIComponent(fieldId)}`, "DELETE"); },
  setProjectCustomValue: async (modelId: string, fieldId: string, value?: string | number | boolean) => {
    const model = await remoteProject(modelId);
    await remoteJson(`/api/v1/projects/${encodeURIComponent(modelId)}/custom-fields/${encodeURIComponent(fieldId)}`, "PUT", { value, ifUnmodifiedSince: model.revision });
  },
  saveBomItem: async (input: { id?: string; modelId: string; kind: "printed" | "purchased"; linkedAssetId?: string; name: string; quantity: number; unit: string; url?: string; notes: string; sortOrder?: number }) => {
    const model = await remoteProject(input.modelId);
    const documentation = await remoteJson<ProjectDocumentation & { revision: string }>(`/api/v1/projects/${encodeURIComponent(input.modelId)}/bom`, "POST", { ...input, ifUnmodifiedSince: model.revision });
    rememberRemotePhotos(input.modelId, documentation);
    const item = input.id ? documentation.bomItems.find((value) => value.id === input.id) : documentation.bomItems.at(-1);
    if (!item) throw new Error("The server did not return the saved BOM item.");
    return item;
  },
  deleteBomItem: async (itemId: string) => {
    const modelId = remoteBomProjects.get(itemId);
    if (!modelId) throw new Error("Refresh this project's documentation before deleting its BOM item.");
    const model = await remoteProject(modelId);
    await remoteJson(`/api/v1/projects/${encodeURIComponent(modelId)}/bom/${encodeURIComponent(itemId)}`, "DELETE", { ifUnmodifiedSince: model.revision });
    remoteBomProjects.delete(itemId);
  },
  saveAssemblyStep: async (input: { id?: string; modelId: string; title: string; instructions: string; sortOrder?: number; bomItemIds: string[]; photoIds: string[] }) => {
    const model = await remoteProject(input.modelId);
    const documentation = await remoteJson<ProjectDocumentation & { revision: string }>(`/api/v1/projects/${encodeURIComponent(input.modelId)}/assembly-steps`, "POST", { ...input, ifUnmodifiedSince: model.revision });
    rememberRemotePhotos(input.modelId, documentation);
    const step = input.id ? documentation.assemblySteps.find((value) => value.id === input.id) : documentation.assemblySteps.at(-1);
    if (!step) throw new Error("The server did not return the saved assembly step.");
    return step;
  },
  deleteAssemblyStep: async (stepId: string) => {
    const modelId = remoteStepProjects.get(stepId);
    if (!modelId) throw new Error("Refresh this project's documentation before deleting its assembly step.");
    const model = await remoteProject(modelId);
    await remoteJson(`/api/v1/projects/${encodeURIComponent(modelId)}/assembly-steps/${encodeURIComponent(stepId)}`, "DELETE", { ifUnmodifiedSince: model.revision });
    remoteStepProjects.delete(stepId);
  },
  savedSearches: () => remoteJson<SavedSearch[]>("/api/v1/saved-searches"),
  saveSavedSearch: (input: { id?: string; name: string; query: ModelQuery }) => remoteJson<SavedSearch>("/api/v1/saved-searches", "POST", input),
  deleteSavedSearch: async (searchId: string) => { await remoteJson(`/api/v1/saved-searches/${encodeURIComponent(searchId)}`, "DELETE"); },
  collections: () => remoteJson<Collection[]>("/api/v1/collections"),
  createCollection: (input: { name: string; symbol: string; color: string; smart?: boolean; rule?: SmartCollectionRule }) => remoteJson<Collection>("/api/v1/collections", "POST", input),
  updateCollection: (collectionId: string, input: { name: string; symbol: string; color: string; smart?: boolean; rule?: SmartCollectionRule }) => remoteJson<Collection>(`/api/v1/collections/${encodeURIComponent(collectionId)}`, "PATCH", input),
  deleteCollection: async (collectionId: string) => { await remoteJson(`/api/v1/collections/${encodeURIComponent(collectionId)}`, "DELETE"); },
  addToCollection: async (collectionId: string, modelIds: string[]) => { for (const id of modelIds) await remoteJson(`/api/v1/collections/${encodeURIComponent(collectionId)}/projects/${encodeURIComponent(id)}`, "PUT"); },
  removeFromCollection: async (collectionId: string, modelIds: string[]) => { for (const id of modelIds) await remoteJson(`/api/v1/collections/${encodeURIComponent(collectionId)}/projects/${encodeURIComponent(id)}`, "DELETE"); },
  tags: () => remoteJson<Tag[]>("/api/v1/tags"),
  saveTag: (input: { id?: string; name: string; color: string }) => input.id
    ? remoteJson<Tag>(`/api/v1/tags/${encodeURIComponent(input.id)}`, "PATCH", { name: input.name, color: input.color })
    : remoteJson<Tag>("/api/v1/tags", "POST", { name: input.name, color: input.color }),
  deleteTag: async (tagId: string) => { await remoteJson(`/api/v1/tags/${encodeURIComponent(tagId)}`, "DELETE"); },
  addToTag: async (tagId: string, modelIds: string[]) => {
    for (const id of modelIds) {
      const model = await remoteProject(id);
      await remoteJson(`/api/v1/projects/${encodeURIComponent(id)}`, "PATCH", { tagIds: [...new Set([...model.tags.map((tag) => tag.id), tagId])], ifUnmodifiedSince: model.revision });
    }
  },
  relatedModels: (modelId: string) => remoteJson<RelatedModel[]>(`/api/v1/projects/${encodeURIComponent(modelId)}/related`),
  duplicateStats: () => remoteJson<DuplicateStats>("/api/v1/duplicates/stats"),
  duplicateGroups: (offset = 0, limit = 48) => remoteJson<Page<DuplicateGroup>>(`/api/v1/duplicates${queryString({ offset, limit })}`),
  webSources: () => remoteJson<WebSource[]>("/api/v1/web-sources"),
  materials: () => remoteJson<Material[]>("/api/v1/materials"),
  saveMaterial: (input: Partial<Material> & Pick<Material, "name" | "materialType" | "spoolPriceMinor" | "currency" | "spoolWeightG">) => remoteJson<Material>("/api/v1/materials", "POST", input),
  saveEstimate: async (input: Record<string, unknown>) => {
    const modelId = String(input.modelId ?? "");
    const model = await remoteProject(modelId);
    await remoteJson(`/api/v1/projects/${encodeURIComponent(modelId)}/estimate`, "PUT", { ...input, ifUnmodifiedSince: model.revision });
  },
  plateThumbnail: (assetId: string, plateIndex: number) => remoteBinary(`/api/v1/assets/${encodeURIComponent(assetId)}/plates/${plateIndex}/thumbnail`),
  viewerMesh: (assetId: string, plateIndex?: number) => remoteBinary(`/api/v1/assets/${encodeURIComponent(assetId)}/viewer-mesh${queryString({ plateIndex })}`),
  thumbnail: (assetId: string) => remoteBinary(`/api/v1/assets/${encodeURIComponent(assetId)}/thumbnail`),
  requestThumbnail: async () => false,
  cancelThumbnail: async () => undefined,
  managedLibrary: async () => {
    const roots = await remoteApi.roots!() as LibraryRoot[];
    const root = roots.find((item) => item.rootKind === "managed");
    return { root, projectsPath: root ? `${root.displayName}/Projects` : undefined, inboxPath: root ? `${root.displayName}/Inbox` : undefined, trashPath: root ? `${root.displayName}/Trash` : undefined };
  },
  libraryAssets: async () => [],
  createProject: async (input: { name: string; filePaths: string[]; assetIds: string[]; transferMode: "move" | "copy" | "link" }) => {
    if (input.assetIds.length) throw new Error("Choose local files when creating a project on a remote server.");
    if (!input.filePaths.length) {
      const project = await remoteJson<ModelDetail & { revision: string }>("/api/v1/projects", "POST", { name: input.name });
      return project.id;
    }
    const first = await remoteUpload(input.filePaths[0], { projectName: input.name });
    for (const path of input.filePaths.slice(1)) await remoteUpload(path, { projectId: first.projectId });
    return first.projectId;
  },
  addProjectFiles: async (input: { projectId: string; filePaths: string[]; assetIds: string[]; transferMode: "move" | "copy" | "link" }) => {
    if (input.assetIds.length) throw new Error("Choose local files to upload to a remote project.");
    const uploaded: string[] = [];
    for (const path of input.filePaths) uploaded.push((await remoteUpload(path, { projectId: input.projectId })).assetId);
    return uploaded;
  },
  openAsset: (assetId: string, appPath?: string) => command<void>("open_remote_asset", { assetId, appPath }),
  revealAsset: (assetId: string) => command<void>("reveal_remote_asset", { assetId }),
  companionStatus: async () => remoteUnavailable("Companion host settings"),
};

const remoteBlocked = new Set<keyof typeof localApi>([
  "addRoot", "removeRoot", "reconnectRoot", "startScan", "pauseScan", "scanStatus",
  "configureManagedLibrary", "importInbox", "renameProjectFile",
  "consolidationPreview", "convertProjectToManaged",
  "inspectProjectPackage", "exportProjectPackage",
  "importProjectPackage",
  "mergeProjects", "splitProject", "setProjectPrimary", "dismissDuplicate", "cleanupDuplicate",
  "preview", "clearPreviewCache", "exportMetadata", "exportDiagnostics", "backupDestinations",
  "saveBackupDestination", "deleteBackupDestination", "testBackupDestination", "runBackup", "backupRuns",
  "backupArchives", "prepareRestoreFromPath", "prepareRestoreFromDestination", "commitPreparedRestore",
  "cancelPreparedRestore", "setCompanionEnabled", "setCompanionOrganizationWrites", "createCompanionPairing",
  "revokeCompanionDevice", "previewWebSource", "saveWebSource", "deleteWebSource", "attachWebSource"
]);

// These commands deliberately remain local while browsing a remote library.
// Everything else must have a remote adapter or be blocked, so a newly added
// Desktop command can never mutate the local database by accident.
const remoteLocalOnly = new Set<keyof typeof localApi>([
  "preferences", "savePreference", "setMenuBarEnabled", "slicers", "openExternal", "calculateCost"
]);

export const api = new Proxy(localApi, {
  get(target, property: keyof typeof localApi) {
    if (activeBackend !== "remote") return target[property];
    if (remoteApi[property]) return remoteApi[property];
    if (remoteBlocked.has(property)) return (..._args: unknown[]) => remoteUnavailable(String(property));
    if (remoteLocalOnly.has(property)) return target[property];
    return (..._args: unknown[]) => remoteUnavailable(String(property));
  }
}) as typeof localApi;

export async function onBackendEvent<T = unknown>(event: string, handler: (payload: T) => void): Promise<UnlistenFn> {
  if (!isTauri()) return () => undefined;
  return listen<T>(event, ({ payload }) => handler(payload));
}
