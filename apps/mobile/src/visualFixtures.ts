import type {
  Collection, CompanionPage, Credential, CustomFieldDefinition, FolderSummary, Material,
  ProjectDetail, ProjectDocumentation, ProjectSummary, RelatedProject, SavedSearch,
  ServerStatus, Tag,
} from "./types";

export const visualCredential: Credential = {
  serverId: "visual-server",
  serverName: "Workshop Mac",
  baseUrl: "http://100.81.102.15:43821",
  deviceId: "visual-device",
  token: "visual-token",
};

export const visualStatus: ServerStatus = {
  serverId: visualCredential.serverId,
  name: visualCredential.serverName,
  protocolVersion: 1,
  projectCount: 589,
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
  { id: "collection-workshop", name: "Workshop", symbol: "boxes", color: "#ff5a36", modelCount: 18, smart: false, createdAt: "2026-10-01T08:00:00Z" },
  { id: "collection-print-next", name: "Print next", symbol: "boxes", color: "#6d7df2", modelCount: 7, smart: false, createdAt: "2026-10-02T08:00:00Z" },
  { id: "collection-favorites", name: "Desk upgrades", symbol: "boxes", color: "#2f8f64", modelCount: 12, smart: true, createdAt: "2026-10-03T08:00:00Z" },
];

export const visualFolders: FolderSummary[] = [
  { id: "folder-workshop", name: "Workshop tools", modelCount: 84 },
  { id: "folder-home", name: "Home", modelCount: 61 },
];

export const visualTags: Tag[] = [
  { id: "tag-functional", name: "Functional", color: "#ff5a36", modelCount: 47 },
  { id: "tag-favorite", name: "Favorite print", color: "#6d7df2", modelCount: 21 },
];

export const visualSavedSearches: SavedSearch[] = [
  { id: "saved-ready", name: "Ready to print", query: { availability: "available", sort: "modified" }, createdAt: "2026-10-04T08:00:00Z", updatedAt: "2026-10-04T08:00:00Z" },
];

const summaries: ProjectSummary[] = [
  {
    id: "project-drill-dock", displayName: "Modular Drill Dock", folderId: "folder-workshop",
    folderName: "Workshop tools", primaryAssetId: "asset-drill", primaryExtension: "3mf",
    favorite: true, addedAt: "2026-10-20T10:00:00Z", modifiedAt: "2026-10-25T09:42:00Z",
    lastOpenedAt: "2026-10-25T09:45:00Z", missing: false, assetCount: 3,
    dimensionsMm: [184, 92, 76],
  },
  {
    id: "project-caliper", displayName: "Caliper Wall Mount", folderId: "folder-workshop",
    folderName: "Workshop tools", primaryAssetId: "asset-caliper", primaryExtension: "stl",
    favorite: false, addedAt: "2026-10-18T10:00:00Z", modifiedAt: "2026-10-24T14:15:00Z",
    lastOpenedAt: "2026-10-24T14:20:00Z", missing: false, assetCount: 1,
    dimensionsMm: [148, 44, 22],
  },
  {
    id: "project-planter", displayName: "Twist Planter", folderId: "folder-home",
    folderName: "Home", primaryAssetId: "asset-planter", primaryExtension: "stl",
    favorite: true, addedAt: "2026-10-15T10:00:00Z", modifiedAt: "2026-10-23T16:05:00Z",
    lastOpenedAt: "2026-10-23T16:06:00Z", missing: false, assetCount: 2,
    dimensionsMm: [122, 122, 138],
  },
  {
    id: "project-lamp", displayName: "Reading Lamp Joint", folderId: "folder-home",
    folderName: "Home", primaryAssetId: "asset-lamp", primaryExtension: "step",
    favorite: false, addedAt: "2026-10-10T10:00:00Z", modifiedAt: "2026-10-22T11:30:00Z",
    lastOpenedAt: "2026-10-22T11:31:00Z", missing: false, assetCount: 4,
    dimensionsMm: [64, 58, 48],
  },
];

export const visualProjects: CompanionPage<ProjectSummary> = {
  items: summaries,
  total: 589,
};

export const visualProject: ProjectDetail = {
  ...summaries[0],
  revision: "project-revision-4",
  notes: "Print the dock face-down with four walls. Test-fit the battery rails before fastening it beneath the workbench.",
  rootName: "modular-drill-dock",
  collectionIds: ["collection-workshop", "collection-print-next"],
  tags: visualTags,
  assets: [
    {
      id: "asset-drill", filename: "drill-dock.3mf", extension: "3mf", byteSize: 18_432_000,
      modifiedAt: "2026-10-25T09:42:00Z", parseStatus: "ready", missing: false, role: "printable",
      metadata: {
        dimensionsMm: [184, 92, 76], triangleCount: 184_620, objectCount: 3,
        materialNames: ["PETG · Safety orange"], filamentGrams: 126.4, surfaceAreaMm2: 42_180,
        volumeMm3: 104_200,
        threeMf: {
          slicer: "OrcaSlicer", slicerVersion: "2.3", printer: "Voron 2.4", printProfile: "0.20 mm Strength",
          nozzleDiameterMm: 0.4, layerHeightMm: 0.2,
          plates: [{ index: 0, name: "Dock + rails", objectCount: 3, objectNames: ["Dock", "Left rail", "Right rail"], bedType: "Textured PEI", filamentGrams: 126.4, printTimeSeconds: 18_540, materialNames: ["PETG"], thumbnail: true }],
        },
      },
    },
    {
      id: "asset-bracket", filename: "bench-bracket.step", extension: "step", byteSize: 2_840_000,
      modifiedAt: "2026-10-24T15:00:00Z", parseStatus: "ready", missing: false, role: "reference",
      metadata: { dimensionsMm: [82, 36, 28], objectCount: 1 },
    },
  ],
  estimate: {
    id: "estimate-drill", modelId: "project-drill-dock", assetId: "asset-drill", materialId: "material-petg",
    plasticG: 126.4, quantity: 1, source: "3mf", costPerPieceMinor: 284, batchCostMinor: 284,
  },
};

export const visualDocumentation: ProjectDocumentation = {
  revision: "docs-revision-2",
  photos: [],
  customValues: [{ fieldId: "field-printer", value: "Voron 2.4", updatedAt: "2026-10-25T09:42:00Z" }],
  bomItems: [
    { id: "bom-screws", modelId: visualProject.id, kind: "purchased", name: "M4 × 20 socket screws", quantity: 4, unit: "pcs", notes: "Black oxide", sortOrder: 0, createdAt: "2026-10-24T09:00:00Z", updatedAt: "2026-10-24T09:00:00Z" },
  ],
  assemblySteps: [
    { id: "step-fit", modelId: visualProject.id, title: "Test-fit the rails", instructions: "Slide the battery into both rails before mounting the dock.", sortOrder: 0, bomItemIds: ["bom-screws"], photoIds: [], createdAt: "2026-10-24T09:00:00Z", updatedAt: "2026-10-24T09:00:00Z" },
  ],
};

export const visualFields: CustomFieldDefinition[] = [
  { id: "field-printer", name: "Printer", fieldType: "text", options: [] },
];

export const visualMaterials: Material[] = [
  { id: "material-petg", name: "PETG · Safety orange", materialType: "PETG", colorName: "Safety orange", colorHex: "#f05a2a", spoolPriceMinor: 2249, currency: "EUR", spoolWeightG: 1000, updatedAt: "2026-10-20T08:00:00Z" },
];

export const visualRelated: RelatedProject[] = [
  { id: "project-caliper", displayName: "Caliper Wall Mount", relativeFolder: "Workshop tools", primaryExtension: "stl", modifiedAt: "2026-10-24T14:15:00Z", relationship: "geometry" },
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
  if (pathname === "/api/v1/projects") return visualProjects;
  return undefined;
}

export function visualMediaEnvelope(path: string) {
  if (path.includes("/viewer-mesh")) {
    return encodeVisualMediaEnvelope(
      new Uint8Array(visualLocalStlEnvelope().slice(8)),
      "application/octet-stream",
      "visual-viewer-vlm5",
    );
  }
  const palettes = [
    ["#ff6a48", "#25251f"], ["#6d7df2", "#29283b"], ["#2f8f64", "#183228"], ["#d29a42", "#33291a"],
  ];
  const hash = [...path].reduce((total, character) => total + character.charCodeAt(0), 0);
  const [accent, background] = palettes[hash % palettes.length];
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 640 480"><defs><linearGradient id="g" x1="0" x2="1" y1="0" y2="1"><stop stop-color="${background}"/><stop offset="1" stop-color="#11110f"/></linearGradient></defs><rect width="640" height="480" fill="url(#g)"/><path d="M182 314V190l138-80 138 80v124l-138 80z" fill="${accent}" opacity=".9"/><path d="m182 190 138 80 138-80M320 270v124" fill="none" stroke="#fff" stroke-opacity=".44" stroke-width="7"/><circle cx="500" cy="86" r="44" fill="${accent}" opacity=".24"/></svg>`;
  return encodeVisualMediaEnvelope(new TextEncoder().encode(svg), "image/svg+xml", `visual-${hash}`);
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

export function visualLocalStlEnvelope() {
  const vertices = [
    [0, 0, 0], [20, 0, 0], [20, 30, 0], [0, 30, 0],
    [0, 0, 20], [20, 0, 20], [20, 30, 20], [0, 30, 20],
    [28, 5, 0], [40, 5, 0], [40, 17, 0], [28, 17, 0],
    [28, 5, 12], [40, 5, 12], [40, 17, 12], [28, 17, 12],
    [-10, -10, -1], [50, -10, -1], [50, 40, -1], [-10, 40, -1],
  ];
  const boxTriangles = (base: number) => [
    [base, base + 2, base + 1], [base, base + 3, base + 2], [base + 4, base + 5, base + 6], [base + 4, base + 6, base + 7],
    [base, base + 1, base + 5], [base, base + 5, base + 4], [base + 1, base + 2, base + 6], [base + 1, base + 6, base + 5],
    [base + 2, base + 3, base + 7], [base + 2, base + 7, base + 6], [base + 3, base, base + 4], [base + 3, base + 4, base + 7],
  ];
  const triangles = [...boxTriangles(0), ...boxTriangles(8), [16, 17, 18], [16, 18, 19]];
  const indexCount = triangles.length * 3;
  const objectCount = 2;
  const headerLength = 40 + objectCount * 32;
  const payloadLength = headerLength + vertices.length * 12 + indexCount * 4 + indexCount + triangles.length + triangles.length * 4;
  const envelope = new ArrayBuffer(8 + payloadLength);
  const bytes = new Uint8Array(envelope);
  const view = new DataView(envelope);
  bytes.set(new TextEncoder().encode("VLP1"), 0);
  view.setUint32(4, 24, true);
  bytes.set(new TextEncoder().encode("VLM5"), 8);
  view.setUint32(12, vertices.length, true);
  view.setUint32(16, indexCount, true);
  [0, 0, 0, 40, 30, 20].forEach((value, index) => view.setFloat32(20 + index * 4, value, true));
  view.setUint32(44, objectCount, true);
  view.setUint32(48, 1, true);
  view.setUint32(52, 12, true);
  [0, 0, 0, 20, 30, 20].forEach((value, index) => view.setFloat32(56 + index * 4, value, true));
  view.setUint32(80, 2, true);
  view.setUint32(84, 12, true);
  [28, 5, 0, 40, 17, 12].forEach((value, index) => view.setFloat32(88 + index * 4, value, true));
  let offset = 8 + headerLength;
  vertices.flat().forEach((value) => { view.setFloat32(offset, value, true); offset += 4; });
  triangles.flat().forEach((value) => { view.setUint32(offset, value, true); offset += 4; });
  triangles.forEach((_, index) => {
    bytes.set(index < 24 ? [216, 200, 170] : [54, 55, 64], offset);
    offset += 3;
  });
  triangles.forEach((_, index) => { bytes[offset++] = index < 24 ? 0 : 1; });
  triangles.forEach((_, index) => {
    view.setUint32(offset, index < 12 ? 1 : index < 24 ? 2 : 0, true);
    offset += 4;
  });
  return envelope;
}
