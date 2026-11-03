import type {
  Collection, CompanionPage, Credential, CustomFieldDefinition, FolderSummary, Material,
  ProjectDetail, ProjectDocumentation, ProjectSummary, RelatedProject, SavedSearch,
  ServerStatus, Tag,
} from "./types";

export const visualCredential: Credential = {
  serverId: "visual-server",
  serverName: "Studio Mac mini",
  baseUrl: "http://100.81.102.15:43821",
  deviceId: "visual-device",
  token: "visual-token",
};

export const visualStatus: ServerStatus = {
  serverId: visualCredential.serverId,
  name: visualCredential.serverName,
  protocolVersion: 1,
  projectCount: 247,
  libraryRevision: "visual-revision",
  capabilities: {
    readLibrary: true,
    organizationWrites: true,
    photoUploads: true,
    fileUploads: true,
    viewerMeshes: true,
    plateThumbnails: true,
    costEstimates: true,
    relatedProjects: true,
    liveEvents: false,
  },
};

export const visualCollections: Collection[] = [
  { id: "collection-calibration", name: "Calibration lab", symbol: "boxes", color: "#3b82f6", modelCount: 14, smart: false, createdAt: "2026-10-01T08:00:00Z" },
  { id: "collection-print-next", name: "Weekend queue", symbol: "boxes", color: "#ff5a36", modelCount: 8, smart: false, createdAt: "2026-10-02T08:00:00Z" },
  { id: "collection-workshop", name: "Workshop upgrades", symbol: "boxes", color: "#21a179", modelCount: 32, smart: true, createdAt: "2026-10-03T08:00:00Z" },
];

export const visualFolders: FolderSummary[] = [
  { id: "folder-calibration", name: "Calibration & profiles", modelCount: 38 },
  { id: "folder-print-in-place", name: "Print-in-place", modelCount: 27 },
  { id: "folder-workshop", name: "Workshop storage", modelCount: 64 },
];

export const visualTags: Tag[] = [
  { id: "tag-calibrated", name: "Calibrated", color: "#3b82f6", modelCount: 31 },
  { id: "tag-no-supports", name: "No supports", color: "#21a179", modelCount: 67 },
  { id: "tag-print-in-place", name: "Print-in-place", color: "#ff5a36", modelCount: 44 },
];

export const visualSavedSearches: SavedSearch[] = [
  { id: "saved-ready", name: "Ready this weekend", query: { availability: "available", sort: "modified" }, createdAt: "2026-10-04T08:00:00Z", updatedAt: "2026-10-04T08:00:00Z" },
];

const summaries: ProjectSummary[] = [
  {
    id: "project-benchy", displayName: "3DBenchy Calibration Boat", folderId: "folder-calibration",
    folderName: "Calibration & profiles", primaryAssetId: "asset-benchy", primaryExtension: "stl",
    favorite: true, addedAt: "2026-10-20T10:00:00Z", modifiedAt: "2026-10-30T09:42:00Z",
    lastOpenedAt: "2026-10-30T09:45:00Z", missing: false, assetCount: 2,
    dimensionsMm: [60, 31, 48],
  },
  {
    id: "project-flexi-rex", displayName: "Flexi Rex · Stronger Links", folderId: "folder-print-in-place",
    folderName: "Print-in-place", primaryAssetId: "asset-flexi-rex", primaryExtension: "stl",
    favorite: true, addedAt: "2026-10-18T10:00:00Z", modifiedAt: "2026-10-28T14:15:00Z",
    lastOpenedAt: "2026-10-28T14:20:00Z", missing: false, assetCount: 1,
    dimensionsMm: [81.3, 67.6, 13],
  },
  {
    id: "project-nasa-wrench", displayName: "NASA ISS Ratchet Wrench", folderId: "folder-workshop",
    folderName: "Workshop storage", primaryAssetId: "asset-nasa-wrench", primaryExtension: "stl",
    favorite: false, addedAt: "2026-10-15T10:00:00Z", modifiedAt: "2026-10-26T16:05:00Z",
    lastOpenedAt: "2026-10-26T16:06:00Z", missing: false, assetCount: 1,
    dimensionsMm: [33.1, 114.2, 26],
  },
  {
    id: "project-stealthburner", displayName: "Voron Stealthburner Main Body", folderId: "folder-workshop",
    folderName: "Workshop storage", primaryAssetId: "asset-stealthburner", primaryExtension: "stl",
    favorite: false, addedAt: "2026-10-10T10:00:00Z", modifiedAt: "2026-10-24T11:30:00Z",
    lastOpenedAt: "2026-10-24T11:31:00Z", missing: false, assetCount: 1,
    dimensionsMm: [65.1, 126.8, 31.8],
  },
];

export const visualProjects: CompanionPage<ProjectSummary> = {
  items: summaries,
  total: 247,
};

export const visualProject: ProjectDetail = {
  ...summaries[0],
  revision: "project-benchy-revision-3",
  notes: "Baseline speed profile: 0.20 mm layers, three walls, and no supports. Check the bow overhang, cabin arches, and stern lettering before saving this printer profile.",
  rootName: "calibration/3dbenchy-speed-profile",
  collectionIds: ["collection-calibration", "collection-print-next"],
  tags: visualTags.slice(0, 2),
  assets: [
    {
      id: "asset-benchy", filename: "3DBenchy.stl", extension: "stl", byteSize: 11_285_384,
      modifiedAt: "2026-10-30T09:42:00Z", parseStatus: "ready", missing: false, role: "printable",
      metadata: {
        dimensionsMm: [60, 31, 48], triangleCount: 225_706, objectCount: 1,
        materialNames: ["PLA Matte · Ocean blue"], filamentGrams: 13.8, surfaceAreaMm2: 8_120,
        volumeMm3: 11_560,
      },
    },
    {
      id: "asset-benchy-profile", filename: "3DBenchy · 0.20 mm Speed.3mf", extension: "3mf", byteSize: 1_842_000,
      modifiedAt: "2026-10-30T09:44:00Z", parseStatus: "ready", missing: false, role: "printable",
      metadata: {
        dimensionsMm: [60, 31, 48], triangleCount: 225_706, objectCount: 1,
        materialNames: ["PLA Matte · Ocean blue"], filamentGrams: 13.8,
        threeMf: {
          slicer: "OrcaSlicer", slicerVersion: "2.3", printer: "Bambu Lab X1C", printProfile: "0.20 mm Speed",
          nozzleDiameterMm: 0.4, layerHeightMm: 0.2,
          plates: [{ index: 0, name: "Speed calibration", objectCount: 1, objectNames: ["3DBenchy"], bedType: "Textured PEI", filamentGrams: 13.8, printTimeSeconds: 3_840, materialNames: ["PLA Matte"], thumbnail: true }],
        },
      },
    },
  ],
  estimate: {
    id: "estimate-benchy", modelId: "project-benchy", assetId: "asset-benchy-profile", materialId: "material-pla-blue",
    plasticG: 13.8, quantity: 1, source: "3mf", costPerPieceMinor: 32, batchCostMinor: 32,
  },
};

export const visualFlexiRexProject: ProjectDetail = {
  ...summaries[1],
  revision: "project-flexi-rex-revision-1",
  notes: "The real print-in-place Flexi Rex with reinforced links by DrLex. Print flat without supports, then gently free each hinge after the build plate cools.",
  rootName: "print-in-place/flexi-rex-stronger-links",
  collectionIds: ["collection-print-next"],
  tags: visualTags.slice(1),
  assets: [{
    id: "asset-flexi-rex", filename: "Flexi-Rex-improved.stl", extension: "stl", byteSize: 745_584,
    modifiedAt: "2026-10-28T14:15:00Z", parseStatus: "ready", missing: false, role: "printable",
    metadata: {
      dimensionsMm: [81.3, 67.6, 13], triangleCount: 14_910, objectCount: 1,
      materialNames: ["PLA · Coral"], filamentGrams: 16.2,
    },
  }],
  estimate: {
    id: "estimate-flexi-rex", modelId: "project-flexi-rex", assetId: "asset-flexi-rex",
    plasticG: 16.2, quantity: 1, source: "stl", costPerPieceMinor: 37, batchCostMinor: 37,
  },
  webSource: {
    provider: "GitHub", canonicalUrl: "https://github.com/DrLex0/print3D-FlexiRex",
    title: "Flexi Rex with stronger links", creator: "DrLex, based on Flexi Rex by Kirbs",
    license: "CC BY-SA 4.0",
  },
};

export const visualDocumentation: ProjectDocumentation = {
  revision: "benchy-docs-revision-2",
  photos: [],
  customValues: [{ fieldId: "field-printer", value: "Bambu Lab X1C · 0.4 mm", updatedAt: "2026-10-30T09:42:00Z" }],
  bomItems: [
    { id: "bom-filament", modelId: visualProject.id, kind: "purchased", name: "PLA Matte · Ocean blue", quantity: 13.8, unit: "g", notes: "Dry at 45 °C before calibration", sortOrder: 0, createdAt: "2026-10-29T09:00:00Z", updatedAt: "2026-10-29T09:00:00Z" },
  ],
  assemblySteps: [
    { id: "step-inspect", modelId: visualProject.id, title: "Inspect the calibration features", instructions: "Check the first layer, bow overhang, cabin arches, chimney, and stern lettering.", sortOrder: 0, bomItemIds: ["bom-filament"], photoIds: [], createdAt: "2026-10-29T09:00:00Z", updatedAt: "2026-10-29T09:00:00Z" },
  ],
};

export const visualFlexiRexDocumentation: ProjectDocumentation = {
  revision: "flexi-rex-docs-revision-1",
  photos: [], customValues: [], bomItems: [], assemblySteps: [],
};

export const visualFields: CustomFieldDefinition[] = [
  { id: "field-printer", name: "Printer", fieldType: "text", options: [] },
];

export const visualMaterials: Material[] = [
  { id: "material-pla-blue", name: "PLA Matte · Ocean blue", materialType: "PLA", colorName: "Ocean blue", colorHex: "#3b82f6", spoolPriceMinor: 2299, currency: "EUR", spoolWeightG: 1000, updatedAt: "2026-10-20T08:00:00Z" },
];

export const visualRelated: RelatedProject[] = [
  { id: "project-flexi-rex", displayName: "Flexi Rex · Stronger Links", relativeFolder: "Print-in-place", primaryExtension: "stl", modifiedAt: "2026-10-28T14:15:00Z", relationship: "geometry" },
];

export function visualJsonResponse(path: string) {
  const pathname = path.split("?")[0];
  if (pathname === "/identity") return { serverId: visualCredential.serverId, protocolMin: 1, protocolMax: 1 };
  if (pathname === "/api/v1/status") return visualStatus;
  if (pathname === "/api/v1/collections") return visualCollections;
  if (pathname === "/api/v1/folders") return visualFolders;
  if (pathname === "/api/v1/saved-searches") return visualSavedSearches;
  if (pathname === "/api/v1/tags") return visualTags;
  if (pathname === "/api/v1/custom-fields") return visualFields;
  if (pathname === "/api/v1/materials") return visualMaterials;
  if (pathname === `/api/v1/projects/${visualProject.id}`) return visualProject;
  if (pathname === `/api/v1/projects/${visualProject.id}/documentation`) return visualDocumentation;
  if (pathname === `/api/v1/projects/${visualProject.id}/related`) return visualRelated;
  if (pathname === `/api/v1/projects/${visualFlexiRexProject.id}`) return visualFlexiRexProject;
  if (pathname === `/api/v1/projects/${visualFlexiRexProject.id}/documentation`) return visualFlexiRexDocumentation;
  if (pathname === `/api/v1/projects/${visualFlexiRexProject.id}/related`) return [];
  if (pathname === "/api/v1/projects") return visualProjects;
  return undefined;
}

const demoAssetRequests = new Map<string, Promise<Uint8Array>>();

function demoAsset(path: string) {
  const cached = demoAssetRequests.get(path);
  if (cached) return cached;
  const request = fetch(path).then(async (response) => {
    if (!response.ok) throw new Error(`Demo asset ${path} returned ${response.status}.`);
    return new Uint8Array(await response.arrayBuffer());
  }).catch((error) => {
    demoAssetRequests.delete(path);
    throw error;
  });
  demoAssetRequests.set(path, request);
  return request;
}

export async function visualMediaEnvelope(path: string) {
  if (path.includes("/viewer-mesh")) {
    const fixture = path.includes("asset-flexi-rex")
      ? { file: "Flexi-Rex-improved.vlm5", etag: "visual-flexi-rex-vlm5" }
      : path.includes("asset-nasa-wrench")
        ? { file: "NASA-Wrench.vlm5", etag: "visual-nasa-wrench-vlm5" }
        : path.includes("asset-stealthburner")
          ? { file: "Voron-Stealthburner-main-body.vlm5", etag: "visual-stealthburner-vlm5" }
          : { file: "3DBenchy.vlm5", etag: "visual-3dbenchy-vlm5" };
    return encodeVisualMediaEnvelope(
      await demoAsset(`/demo/${fixture.file}`),
      "application/octet-stream",
      fixture.etag,
    );
  }
  const cover = path.includes("/project-benchy/cover") || path.includes("/asset-benchy-profile/plates/")
    ? { file: "3DBenchy-cover.png", etag: "visual-3dbenchy-cover" }
    : path.includes("/project-flexi-rex/cover")
      ? { file: "Flexi-Rex-cover.png", etag: "visual-flexi-rex-cover" }
      : path.includes("/project-nasa-wrench/cover")
        ? { file: "NASA-Wrench-cover.png", etag: "visual-nasa-wrench-cover" }
        : path.includes("/project-stealthburner/cover")
          ? { file: "Voron-Stealthburner-cover.png", etag: "visual-stealthburner-cover" }
          : undefined;
  if (cover) {
    return encodeVisualMediaEnvelope(
      await demoAsset(`/demo/${cover.file}`),
      "image/png",
      cover.etag,
    );
  }
  return encodeVisualMediaEnvelope(
    await demoAsset("/demo/3DBenchy-cover.png"),
    "image/png",
    "visual-real-model-fallback",
  );
}

function encodeVisualMediaEnvelope(body: Uint8Array, contentTypeValue: string, etagValue: string) {
  const encoder = new TextEncoder();
  const etag = encoder.encode(etagValue);
  const contentType = encoder.encode(contentTypeValue);
  const envelope = new Uint8Array(10 + etag.length + contentType.length + body.length);
  envelope.set(encoder.encode("VMC1"), 0);
  const view = new DataView(envelope.buffer);
  view.setUint16(4, 200, true);
  view.setUint16(6, etag.length, true);
  view.setUint16(8, contentType.length, true);
  envelope.set(etag, 10);
  envelope.set(contentType, 10 + etag.length);
  envelope.set(body, 10 + etag.length + contentType.length);
  return envelope.buffer;
}

export async function visualLocalStlEnvelope(model = "benchy") {
  const file = model === "flexi-rex" ? "Flexi-Rex-improved.vlm5"
    : model === "nasa-wrench" ? "NASA-Wrench.vlm5"
      : model === "stealthburner" ? "Voron-Stealthburner-main-body.vlm5"
        : "3DBenchy.vlm5";
  const payload = await demoAsset(`/demo/${file}`);
  const envelope = new ArrayBuffer(8 + payload.byteLength);
  const bytes = new Uint8Array(envelope);
  bytes.set(new TextEncoder().encode("VLP1"), 0);
  new DataView(envelope).setUint32(4, new DataView(payload.buffer, payload.byteOffset, payload.byteLength).getUint32(8, true) / 3, true);
  bytes.set(payload, 8);
  return envelope;
}
