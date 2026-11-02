import type {
  BackupDestination,
  BackupRun,
  Collection,
  CompanionStatus,
  CustomFieldDefinition,
  DuplicateGroup,
  Folder,
  LibraryRoot,
  Material,
  ModelDetail,
  ModelQuery,
  ModelSummary,
  Page,
  PreviewPayload,
  ProjectDocumentation,
  RemoteMirrorSettings,
  ScanStatus,
  SavedSearch,
  SlicerApplication,
  Tag,
  WebSource,
  WebSourcePreview
} from "../../types";

const now = Date.now();
const folders: Folder[] = [
  { id: "f-workshop", rootId: "demo-root", relativePath: "Workshop", name: "Workshop", modelCount: 4 },
  { id: "f-home", rootId: "demo-root", relativePath: "Home", name: "Home", modelCount: 3 },
  { id: "f-toys", rootId: "demo-root", relativePath: "Toys & Games", name: "Toys & Games", modelCount: 2 },
  { id: "f-parts", rootId: "demo-root", parentId: "f-workshop", relativePath: "Workshop/Parts", name: "Parts", modelCount: 2 }
];

const names = [
  ["Contour Lamp", "3mf", "Home", "f-home", [142, 142, 228]],
  ["Tool Wall Bracket", "stl", "Workshop", "f-workshop", [80, 42, 64]],
  ["Arc Planter", "obj", "Home", "f-home", [168, 168, 126]],
  ["Cable Comb Set", "stl", "Parts", "f-parts", [92, 18, 12]],
  ["Mechanical Iris", "3mf", "Toys & Games", "f-toys", [116, 116, 22]],
  ["Desk Tray", "step", "Home", "f-home", [242, 162, 28]],
  ["Camera Mount", "stl", "Workshop", "f-workshop", [68, 54, 77]],
  ["Hex Bit Index", "3mf", "Parts", "f-parts", [132, 64, 34]],
  ["Balancing Bird", "obj", "Toys & Games", "f-toys", [185, 44, 92]]
] as const;

let models: ModelSummary[] = names.map(([displayName, primaryExtension, folderName, folderId, dimensionsMm], index) => ({
  id: `demo-${index + 1}`,
  displayName,
  folderId,
  folderName,
  relativeFolder: folders.find((folder) => folder.id === folderId)?.relativePath ?? folderName,
  primaryAssetId: `asset-${index + 1}`,
  primaryExtension,
  favorite: [0, 2, 4].includes(index),
  addedAt: new Date(now - index * 86_400_000 * 3).toISOString(),
  modifiedAt: new Date(now - index * 86_400_000).toISOString(),
  lastOpenedAt: index < 5 ? new Date(now - index * 3_600_000).toISOString() : undefined,
  missing: false,
  assetCount: index % 3 === 0 ? 2 : 1,
  bundleMode: "automatic",
  storageMode: "linked",
  dimensionsMm: dimensionsMm as [number, number, number]
}));

let collections: Collection[] = [
  { id: "c-print", name: "To print", symbol: "sparkles", color: "#ff5a36", modelCount: 4, createdAt: new Date().toISOString(), smart: false },
  { id: "c-gifts", name: "Gifts", symbol: "gift", color: "#8f6ac8", modelCount: 2, createdAt: new Date().toISOString(), smart: false },
  { id: "c-garage", name: "Garage", symbol: "wrench", color: "#2f8f64", modelCount: 3, createdAt: new Date().toISOString(), smart: false }
];

let tags: Tag[] = [
  { id: "tag-functional", name: "Functional", color: "#2f8f64", modelCount: 3 },
  { id: "tag-gift", name: "Gift", color: "#8f6ac8", modelCount: 2 }
];
const tagMemberships: Record<string, string[]> = { "demo-1": ["tag-gift"], "demo-2": ["tag-functional"], "demo-4": ["tag-functional"] };

let materials: Material[] = [
  { id: "m-pla", name: "Matte PLA", materialType: "PLA", colorName: "Bone", colorHex: "#e9e2d3", spoolPriceMinor: 1599, currency: "EUR", spoolWeightG: 1000, density: 1.24, updatedAt: new Date().toISOString() }
];

let webSources: WebSource[] = [];
let preferences: Record<string, string> = {};
let backupDestinations: BackupDestination[] = [];
let backupRuns: BackupRun[] = [];
let savedSearches: SavedSearch[] = [];
let remoteMirror: RemoteMirrorSettings = {
  enabled: false,
  mode: "all",
  projectIds: [],
  excludedProjectIds: [],
  folderIds: [],
  collectionIds: [],
};
let companionStatus: CompanionStatus = {
  enabled: false,
  running: false,
  serverId: "demo-companion-server",
  port: 43821,
  organizationWrites: false,
  networkMode: "automatic",
  devices: []
};
let customFields: CustomFieldDefinition[] = [
  { id: "field-status", name: "Build status", fieldType: "select", options: ["Idea", "Printing", "Complete"], sortOrder: 1, createdAt: new Date().toISOString(), updatedAt: new Date().toISOString() },
  { id: "field-hours", name: "Assembly time", fieldType: "number", unit: "h", options: [], sortOrder: 2, createdAt: new Date().toISOString(), updatedAt: new Date().toISOString() }
];
const projectDocs: Record<string, ProjectDocumentation> = Object.fromEntries(models.map((model) => [model.id, { photos: [], customValues: model.id === "demo-1" ? [{ fieldId: "field-status", value: "Complete", updatedAt: new Date().toISOString() }] : [], bomItems: [], assemblySteps: [] }]));

const memberships: Record<string, string[]> = {
  "demo-1": ["c-print", "c-gifts"], "demo-2": ["c-garage"], "demo-4": ["c-print", "c-garage"],
  "demo-5": ["c-gifts"], "demo-7": ["c-garage"], "demo-8": ["c-print"], "demo-9": ["c-print"]
};

export const demoRoot: LibraryRoot = {
  id: "demo-root", path: "/Users/you/3D Models", displayName: "3D Models", status: "online",
  lastScanAt: new Date().toISOString(), modelCount: models.length, rootKind: "linked"
};
let managedRoot: LibraryRoot | undefined = {
  id: "managed-root", path: "/Users/you/Volum", displayName: "Volum", status: "online",
  lastScanAt: new Date().toISOString(), modelCount: 0, rootKind: "managed"
};

export async function mockInvoke<T>(command: string, args: Record<string, unknown> = {}): Promise<T> {
  await new Promise((resolve) => setTimeout(resolve, command === "list_models" ? 80 : 25));
  switch (command) {
    case "get_backend_connection": return { mode: "local", configured: false, online: true, mirrorEnabled: false } as T;
    case "get_remote_mirror_settings": return remoteMirror as T;
    case "configure_remote_mirror": {
      remoteMirror = { ...remoteMirror, enabled: Boolean(args.enabled), path: args.path ? String(args.path) : remoteMirror.path, mode: args.mode === "selected" ? "selected" : remoteMirror.mode };
      return { mode: "remote", configured: true, online: true, mirrorEnabled: remoteMirror.enabled, mirrorPath: remoteMirror.path, mirrorMode: remoteMirror.mode } as T;
    }
    case "set_remote_mirror_mode": remoteMirror = { ...remoteMirror, mode: args.mode === "selected" ? "selected" : "all" }; return remoteMirror as T;
    case "set_remote_mirror_selection": remoteMirror = { ...remoteMirror, ...(args.input as RemoteMirrorSettings) }; return remoteMirror as T;
    case "sync_remote_mirror": return { downloaded: 0, uploaded: 0, removed: 0, conflicts: 0, unchanged: 0, totalAssets: 0, cursor: 0, completedAt: new Date().toISOString() } as T;
    case "list_mirrored_projects": return [] as T;
    case "list_mirrored_assets": return [] as T;
    case "get_companion_status": return companionStatus as T;
    case "set_companion_enabled": {
      const enabled = Boolean(args.enabled);
      companionStatus = {
        ...companionStatus,
        enabled,
        running: enabled,
        tailnetIp: enabled ? "100.64.0.10" : undefined,
        dnsName: enabled ? "volum-desktop.example.ts.net" : undefined,
        baseUrl: enabled ? "http://100.64.0.10:43821" : undefined
      };
      return companionStatus as T;
    }
    case "set_companion_organization_writes": {
      companionStatus = { ...companionStatus, organizationWrites: Boolean(args.enabled) };
      return companionStatus as T;
    }
    case "set_companion_network": {
      const networkMode = args.mode === "custom" ? "custom" : "automatic";
      companionStatus = {
        ...companionStatus,
        networkMode,
        bindAddress: networkMode === "custom" ? String(args.bindAddress ?? "") : undefined,
        publicUrl: networkMode === "custom" ? String(args.publicUrl ?? "") : undefined,
        baseUrl: networkMode === "custom" ? String(args.publicUrl ?? "") : companionStatus.baseUrl,
        transport: networkMode === "custom" ? (String(args.publicUrl ?? "").startsWith("https://") ? "https" : "private-http") : "tailscale-http",
      };
      return companionStatus as T;
    }
    case "create_companion_pairing": {
      const expiresAt = new Date(Date.now() + 300_000).toISOString();
      const transport = companionStatus.transport ?? "tailscale-http";
      const payload = { version: 1, serverId: companionStatus.serverId, serverName: "volum-desktop", baseUrl: companionStatus.baseUrl, secret: "demo-pairing-secret", expiresAt, transport };
      return { serverId: companionStatus.serverId, serverName: "volum-desktop", baseUrl: companionStatus.baseUrl, secret: payload.secret, expiresAt, transport, qrPayload: JSON.stringify(payload) } as T;
    }
    case "revoke_companion_device": companionStatus = { ...companionStatus, devices: companionStatus.devices.filter((device) => device.id !== args.deviceId) }; return companionStatus as T;
    case "list_roots": return [...(managedRoot ? [managedRoot] : []), demoRoot] as T;
    case "add_library_root": return demoRoot as T;
    case "remove_library_root": return undefined as T;
    case "reconnect_library_root": return { ...demoRoot, path: String(args.path), status: "online" } as T;
    case "start_scan": return undefined as T;
    case "pause_scan": return undefined as T;
    case "get_managed_library_status": return { root: managedRoot, projectsPath: managedRoot ? `${managedRoot.path}/Projects` : undefined, inboxPath: managedRoot ? `${managedRoot.path}/Inbox` : undefined, trashPath: managedRoot ? `${managedRoot.path}/Trash` : undefined } as T;
    case "configure_managed_library": managedRoot = { id: "managed-root", path: String(args.path), displayName: String(args.path).split(/[\\/]/).pop() || "Volum", status: "online", modelCount: 0, rootKind: "managed" }; return managedRoot as T;
    case "list_library_assets": return models.flatMap((model) => model.primaryAssetId ? [{ id: model.primaryAssetId, projectId: model.id, projectName: model.displayName, filename: `${model.displayName}.${model.primaryExtension}`, extension: model.primaryExtension, byteSize: 1_000_000, missing: model.missing, storageMode: model.storageMode, rootName: demoRoot.displayName, relativePath: model.relativeFolder }] : []) as T;
    case "import_managed_inbox": return { projectIds: [], skipped: 0, errors: [] } as T;
    case "create_managed_project": {
      const input = args.input as { name: string; assetIds: string[]; filePaths: string[]; transferMode: "move" | "copy" | "link" };
      const id = `demo-${crypto.randomUUID()}`;
      const model: ModelSummary = { id, displayName: input.name, folderId: "managed-projects", folderName: input.name, relativeFolder: `Projects/${input.name}`, primaryAssetId: input.assetIds[0], primaryExtension: input.filePaths[0]?.split(".").pop()?.toLowerCase() ?? (input.assetIds.length ? "stl" : ""), favorite: false, addedAt: new Date().toISOString(), modifiedAt: new Date().toISOString(), missing: false, assetCount: input.assetIds.length + input.filePaths.length, bundleMode: "manual", storageMode: input.transferMode === "link" ? "linked" : "managed" };
      models = [model, ...models];
      return id as T;
    }
    case "add_files_to_project": return [] as T;
    case "rename_project": models = models.map((model) => model.id === args.projectId ? { ...model, displayName: String(args.name) } : model); return undefined as T;
    case "rename_project_file": return undefined as T;
    case "preview_project_consolidation": { const model = models.find((item) => item.id === args.projectId)!; return { projectId: model.id, projectName: model.displayName, fileCount: model.assetCount, totalBytes: model.assetCount * 1_000_000, sourceFolders: [model.relativeFolder], managedFolderName: model.displayName, alreadyManaged: model.storageMode === "managed" } as T; }
    case "convert_project_to_managed": { const input = args.input as { projectId: string }; models = models.map((model) => model.id === input.projectId ? { ...model, storageMode: "managed", relativeFolder: `Projects/${model.displayName}` } : model); return undefined as T; }
    case "trash_project": return undefined as T;
    case "restore_project_from_trash": return undefined as T;
    case "delete_project_permanently": models = models.filter((model) => model.id !== args.projectId); return undefined as T;
    case "list_folders": return folders as T;
    case "list_models": {
      const query = (args.query ?? {}) as ModelQuery;
      let output = [...models];
      if (query.search) {
        const search = query.search.toLocaleLowerCase();
        output = output.filter((model) => `${model.displayName} ${model.folderName} ${model.primaryExtension}`.toLocaleLowerCase().includes(search));
      }
      if (query.folderId) output = output.filter((model) => model.folderId === query.folderId);
      if (query.collectionId) output = output.filter((model) => memberships[model.id]?.includes(query.collectionId!));
      if (query.favorite) output = output.filter((model) => model.favorite);
      if (query.recent) output = output.filter((model) => model.lastOpenedAt);
      if (query.format) output = output.filter((model) => model.primaryExtension === query.format);
      if (query.tagId) output = output.filter((model) => tagMemberships[model.id]?.includes(query.tagId!));
      if (query.dateFrom) output = output.filter((model) => (query.dateField === "added" ? model.addedAt : model.modifiedAt).slice(0, 10) >= query.dateFrom!);
      if (query.dateTo) output = output.filter((model) => (query.dateField === "added" ? model.addedAt : model.modifiedAt).slice(0, 10) <= query.dateTo!);
      if (query.duplicates) output = output.filter((model) => ["demo-2", "demo-7"].includes(model.id));
      output.sort((a, b) => {
        if (query.sort === "name") return a.displayName.localeCompare(b.displayName) || a.id.localeCompare(b.id);
        const field = query.sort === "added" ? "addedAt" : query.sort === "opened" ? "lastOpenedAt" : "modifiedAt";
        return (b[field] ?? "").localeCompare(a[field] ?? "") || a.displayName.localeCompare(b.displayName) || a.id.localeCompare(b.id);
      });
      const offset = query.cursor?.startsWith("mock:") ? Number(query.cursor.slice(5)) || 0 : query.offset ?? 0;
      const limit = query.limit ?? 100;
      return { items: output.slice(offset, offset + limit), total: output.length, nextCursor: offset + limit < output.length ? `mock:${offset + limit}` : undefined } as Page<ModelSummary> as T;
    }
    case "get_model": {
      const model = models.find((item) => item.id === args.modelId)!;
      return {
        ...model,
        notes: model.id === "demo-1" ? "Print in vase mode with a warm LED bulb." : "",
        rootId: demoRoot.id, rootName: demoRoot.displayName, rootPath: demoRoot.path,
        collectionIds: memberships[model.id] ?? [],
        tags: tags.filter((tag) => tagMemberships[model.id]?.includes(tag.id)),
        webSource: webSources.find((source) => source.modelId === model.id),
         assets: [{ id: model.primaryAssetId!, filename: `${model.displayName.toLocaleLowerCase().replaceAll(" ", "-")}.${model.primaryExtension}`, extension: model.primaryExtension, relativePath: `${model.relativeFolder}/${model.displayName}.${model.primaryExtension}`, byteSize: 1_842_650 + Number(model.id.split("-")[1]) * 124_000, modifiedAt: model.modifiedAt, parseStatus: "ready", metadata: { dimensionsMm: model.dimensionsMm, triangleCount: 48240, ...(model.primaryExtension === "3mf" ? { threeMf: { slicer: "Bambu Studio", printer: "Bambu Lab A1", printProfile: "0.20mm Standard", nozzleDiameterMm: .4, layerHeightMm: .2, plates: [{ index: 1, objectCount: 2, objectNames: [model.displayName, "Support"], filamentGrams: 42.3, printTimeSeconds: 7320, materialNames: ["PLA"], bedType: "Textured Plate", thumbnail: false }] } } : {}) }, missing: false, role: "printable" }]
      } as T;
    }
    case "list_collections": return collections as T;
    case "create_collection": {
      const input = args.input as { name: string; symbol?: string; color?: string; smart?: boolean; rule?: Collection["rule"] };
      const collection: Collection = { id: crypto.randomUUID(), name: input.name, symbol: input.symbol ?? "box", color: input.color ?? "#ff5a36", modelCount: 0, createdAt: new Date().toISOString(), smart: Boolean(input.smart), rule: input.rule };
      collections = [...collections, collection];
      return collection as T;
    }
    case "delete_collection": collections = collections.filter((item) => item.id !== args.collectionId); return undefined as T;
    case "add_models_to_collection": {
      const modelIds = args.modelIds as string[];
      const collectionId = args.collectionId as string;
      modelIds.forEach((id) => memberships[id] = [...new Set([...(memberships[id] ?? []), collectionId])]);
      return undefined as T;
    }
    case "remove_models_from_collection": {
      const modelIds = args.modelIds as string[];
      modelIds.forEach((id) => memberships[id] = (memberships[id] ?? []).filter((value) => value !== args.collectionId));
      return undefined as T;
    }
    case "toggle_favorite": {
      models = models.map((model) => model.id === args.modelId ? { ...model, favorite: !model.favorite } : model);
      return models.find((model) => model.id === args.modelId)!.favorite as T;
    }
    case "set_favorite": {
      const ids = args.modelIds as string[];
      models = models.map((model) => ids.includes(model.id) ? { ...model, favorite: Boolean(args.favorite) } : model);
      return undefined as T;
    }
    case "save_notes": return undefined as T;
    case "get_project_documentation": return (projectDocs[String(args.modelId)] ?? { photos: [], customValues: [], bomItems: [], assemblySteps: [] }) as T;
    case "list_custom_field_definitions": return customFields as T;
    case "save_custom_field_definition": {
      const input = args.input as { id?: string; name: string; fieldType: CustomFieldDefinition["fieldType"]; unit?: string; options: string[] };
      const timestamp = new Date().toISOString();
      const previous = customFields.find((field) => field.id === input.id);
      const field: CustomFieldDefinition = { ...input, id: input.id ?? crypto.randomUUID(), sortOrder: previous?.sortOrder ?? customFields.length + 1, createdAt: previous?.createdAt ?? timestamp, updatedAt: timestamp };
      customFields = [...customFields.filter((item) => item.id !== field.id), field];
      return field as T;
    }
    case "delete_custom_field_definition": customFields = customFields.filter((field) => field.id !== args.fieldId); Object.values(projectDocs).forEach((docs) => docs.customValues = docs.customValues.filter((value) => value.fieldId !== args.fieldId)); return undefined as T;
    case "set_project_custom_value": {
      const input = args.input as { modelId: string; fieldId: string; value?: string | number | boolean };
      const docs = projectDocs[input.modelId] ??= { photos: [], customValues: [], bomItems: [], assemblySteps: [] };
      docs.customValues = docs.customValues.filter((value) => value.fieldId !== input.fieldId);
      if (input.value != null) docs.customValues.push({ fieldId: input.fieldId, value: input.value, updatedAt: new Date().toISOString() });
      return undefined as T;
    }
    case "save_project_bom_item": {
      const input = args.input as Record<string, unknown> & { modelId: string; id?: string };
      const docs = projectDocs[input.modelId] ??= { photos: [], customValues: [], bomItems: [], assemblySteps: [] };
      const timestamp = new Date().toISOString();
      const item = { ...input, id: input.id ?? crypto.randomUUID(), sortOrder: Number(input.sortOrder ?? docs.bomItems.length + 1), createdAt: timestamp, updatedAt: timestamp };
      docs.bomItems = [...docs.bomItems.filter((value) => value.id !== item.id), item as ProjectDocumentation["bomItems"][number]].sort((a, b) => a.sortOrder - b.sortOrder);
      return item as T;
    }
    case "delete_project_bom_item": Object.values(projectDocs).forEach((docs) => docs.bomItems = docs.bomItems.filter((item) => item.id !== args.itemId)); return undefined as T;
    case "save_assembly_step": {
      const input = args.input as Record<string, unknown> & { modelId: string; id?: string };
      const docs = projectDocs[input.modelId] ??= { photos: [], customValues: [], bomItems: [], assemblySteps: [] };
      const timestamp = new Date().toISOString();
      const step = { ...input, id: input.id ?? crypto.randomUUID(), sortOrder: Number(input.sortOrder ?? docs.assemblySteps.length + 1), createdAt: timestamp, updatedAt: timestamp };
      docs.assemblySteps = [...docs.assemblySteps.filter((value) => value.id !== step.id), step as ProjectDocumentation["assemblySteps"][number]].sort((a, b) => a.sortOrder - b.sortOrder);
      return step as T;
    }
    case "delete_assembly_step": Object.values(projectDocs).forEach((docs) => docs.assemblySteps = docs.assemblySteps.filter((step) => step.id !== args.stepId)); return undefined as T;
    case "get_project_photo": return new ArrayBuffer(0) as T;
    case "inspect_project_package": return { formatVersion: 1, appVersion: "0.2.0", packageProjectId: "demo-package", packageExportId: crypto.randomUUID(), projectName: "Portable demo project", exportedAt: new Date().toISOString(), fileCount: 2, photoCount: 3, bomItemCount: 4, assemblyStepCount: 2, totalBytes: 4_200_000, alreadyImported: false } as T;
    case "export_project_package": return undefined as T;
    case "import_project_package": return "demo-1" as T;
    case "list_saved_searches": return savedSearches as T;
    case "save_saved_search": {
      const input = args.input as { id?: string; name: string; query: ModelQuery };
      const timestamp = new Date().toISOString();
      const search: SavedSearch = { id: input.id ?? crypto.randomUUID(), name: input.name, query: input.query, createdAt: timestamp, updatedAt: timestamp };
      savedSearches = [...savedSearches.filter((item) => item.id !== search.id), search];
      return search as T;
    }
    case "delete_saved_search": savedSearches = savedSearches.filter((item) => item.id !== args.searchId); return undefined as T;
    case "list_tags": return tags as T;
    case "save_tag": {
      const input = args.input as { id?: string; name: string; color: string };
      const tag = { ...input, id: input.id ?? crypto.randomUUID(), modelCount: 0 } as Tag;
      tags = [...tags.filter((item) => item.id !== tag.id), tag];
      return tag as T;
    }
    case "set_model_tags": tagMemberships[String(args.modelId)] = args.tagIds as string[]; return undefined as T;
    case "add_models_to_tag": (args.modelIds as string[]).forEach((id) => tagMemberships[id] = [...new Set([...(tagMemberships[id] ?? []), String(args.tagId)])]); return undefined as T;
    case "list_related_models": return args.modelId === "demo-2" ? [{ id: "demo-7", displayName: "Camera Mount", relativeFolder: "Workshop", primaryExtension: "stl", modifiedAt: models[6].modifiedAt, relationship: "duplicate" }] as T : [] as T;
    case "get_duplicate_stats": return { groups: 1, models: 2, redundantCopies: 1 } as T;
    case "list_duplicate_groups": return { items: [{ id: "exact:demo-duplicate", matchKey: "exact:demo-duplicate", matchKind: "exact", confidence: 1, modelCount: 2, byteSize: 2_100_000, models: [models[1], models[6]] }], total: 1 } as Page<DuplicateGroup> as T;
    case "split_project": return "demo-split" as T;
    case "merge_projects":
    case "set_project_primary_asset":
    case "dismiss_duplicate_match":
    case "cleanup_duplicate_group": return undefined as T;
    case "list_materials": return materials as T;
    case "preview_web_source": {
      const url = String(args.url);
      const provider = url.includes("makerworld.com") ? "makerworld" : "printables";
      return { provider, remoteId: "12345", canonicalUrl: url, title: provider === "makerworld" ? "Modular Desk Organizer" : "Useful Wall Bracket", creator: "Demo Maker", description: "A practical printable model saved from the web.", license: "CC BY 4.0", filamentGrams: provider === "makerworld" ? 42.3 : undefined } as WebSourcePreview as T;
    }
    case "list_web_sources": return webSources as T;
    case "save_web_source": {
      const preview = args.preview as WebSourcePreview;
      const existing = webSources.find((source) => source.canonicalUrl === preview.canonicalUrl);
      const timestamp = new Date().toISOString();
      const source: WebSource = { ...preview, id: existing?.id ?? crypto.randomUUID(), status: existing?.status ?? "saved", rootId: existing?.rootId, relativePath: existing?.relativePath, modelId: existing?.modelId, fetchedAt: timestamp, createdAt: existing?.createdAt ?? timestamp, updatedAt: timestamp };
      webSources = [...webSources.filter((item) => item.id !== source.id), source];
      return source as T;
    }
    case "delete_web_source": webSources = webSources.filter((source) => source.id !== args.sourceId); return undefined as T;
    case "attach_web_source_file": {
      const source = webSources.find((item) => item.id === args.sourceId)!;
      const attached = { ...source, status: "importing", rootId: demoRoot.id, relativePath: `Web Imports/${source.provider}/${source.title}/download.3mf`, updatedAt: new Date().toISOString() } as WebSource;
      webSources = webSources.map((item) => item.id === attached.id ? attached : item);
      return attached as T;
    }
    case "save_material": {
      const input = args.input as Omit<Material, "id" | "updatedAt"> & { id?: string };
      const material = { ...input, id: input.id ?? crypto.randomUUID(), updatedAt: new Date().toISOString() } as Material;
      materials = [...materials.filter((item) => item.id !== material.id), material];
      return material as T;
    }
    case "get_scan_status": return { rootId: args.rootId, state: "complete", discovered: models.length, processed: models.length, errors: 0 } as T;
    case "get_preferences": return preferences as T;
    case "clear_preview_cache": return undefined as T;
    case "list_slicer_apps": return [
      { id: "bambu-studio", name: "Bambu Studio", path: "/Applications/BambuStudio.app", installed: true, custom: false, brandColor: "#00ae42" },
      { id: "orca-slicer", name: "OrcaSlicer", installed: false, custom: false, brandColor: "#1f9ad6" },
      { id: "prusa-slicer", name: "PrusaSlicer", installed: false, custom: false, brandColor: "#f58220" },
      { id: "ultimaker-cura", name: "UltiMaker Cura", installed: false, custom: false, brandColor: "#00a1e0" }
    ] as SlicerApplication[] as T;
    case "request_thumbnail": return false as T;
    case "cancel_thumbnail_request": return undefined as T;
    case "get_cached_thumbnail": return new ArrayBuffer(0) as T;
    case "get_viewer_mesh": return new ArrayBuffer(0) as T;
    case "save_preference": preferences = { ...preferences, [String(args.key)]: String(args.value) }; return undefined as T;
    case "set_menu_bar_enabled": {
      const enabled = Boolean(args.enabled);
      preferences = { ...preferences, menu_bar_enabled: String(enabled) };
      return enabled as T;
    }
    case "list_backup_destinations": return backupDestinations as T;
    case "save_backup_destination": {
      const input = args.input as Omit<BackupDestination, "id" | "credentialSaved" | "createdAt" | "updatedAt"> & { id?: string; password?: string };
      const existing = backupDestinations.find((destination) => destination.id === input.id);
      const timestamp = new Date().toISOString();
      const destination: BackupDestination = {
        id: input.id ?? crypto.randomUUID(), name: input.name, kind: input.kind, location: input.location,
        username: input.username, credentialSaved: Boolean(input.password) || existing?.credentialSaved || false,
        schedule: input.schedule, retentionCount: input.retentionCount, enabled: input.enabled,
        createdAt: existing?.createdAt ?? timestamp, updatedAt: timestamp
      };
      backupDestinations = [...backupDestinations.filter((item) => item.id !== destination.id), destination];
      return destination as T;
    }
    case "delete_backup_destination": backupDestinations = backupDestinations.filter((destination) => destination.id !== args.destinationId); return undefined as T;
    case "test_backup_destination": return undefined as T;
    case "run_backup": {
      const selected = args.destinationId ? backupDestinations.filter((destination) => destination.id === args.destinationId) : backupDestinations;
      const timestamp = new Date().toISOString();
      const runs = selected.map((destination) => ({ id: crypto.randomUUID(), destinationId: destination.id, destinationName: destination.name, reason: "manual", status: "complete", filename: `volum-backup-${timestamp.slice(0, 10).replaceAll("-", "")}.zip`, byteSize: 2_400_000, startedAt: timestamp, completedAt: timestamp } as BackupRun));
      backupRuns = [...runs, ...backupRuns];
      backupDestinations = backupDestinations.map((destination) => selected.some((item) => item.id === destination.id) ? { ...destination, lastAttemptAt: timestamp, lastSuccessAt: timestamp, lastError: undefined } : destination);
      return runs as T;
    }
    case "list_backup_runs": return backupRuns as T;
    case "list_destination_archives": return backupRuns.filter((run) => run.destinationId === args.destinationId && run.status === "complete").map((run) => ({ key: run.filename, filename: run.filename, byteSize: run.byteSize, modifiedAt: run.completedAt })) as T;
    case "prepare_restore_from_path":
    case "prepare_restore_from_destination": return { token: crypto.randomUUID(), appVersion: "0.2.0", createdAt: new Date().toISOString(), byteSize: 2_400_000, counts: { libraries: 1, models: models.length, collections: collections.length, tags: tags.length, webSources: webSources.length } } as T;
    case "commit_prepared_restore": return undefined as T;
    case "cancel_prepared_restore": return undefined as T;
    case "open_external_url": window.open(String(args.url), "_blank", "noopener,noreferrer"); return undefined as T;
    case "calculate_cost": {
      const { plasticG, quantity, spoolPriceMinor, spoolWeightG } = args.input as Record<string, number>;
      const each = plasticG / spoolWeightG * spoolPriceMinor;
      return { costPerPieceMinor: each, batchCostMinor: each * quantity } as T;
    }
    case "get_preview_payload": return { extension: "", bytes: [], sourceAssetId: args.assetId } as PreviewPayload as T;
    case "get_3mf_plate_thumbnail": return new ArrayBuffer(0) as T;
    default: return undefined as T;
  }
}
