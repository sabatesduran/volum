export interface PairingPayload {
  version: number;
  serverId: string;
  serverName: string;
  baseUrl: string;
  secret: string;
  expiresAt: string;
  transport: "tailscale-http";
}

export interface Credential {
  serverId: string;
  serverName: string;
  baseUrl: string;
  deviceId: string;
  token: string;
}

export interface ServerStatus {
  serverId: string;
  name: string;
  protocolVersion: number;
  projectCount: number;
  libraryRevision: string;
  capabilities: {
    readLibrary: boolean;
    organizationWrites: boolean;
    photoUploads: boolean;
    fileUploads: boolean;
    viewerMeshes: boolean;
    plateThumbnails: boolean;
    costEstimates: boolean;
    relatedProjects: boolean;
    liveEvents: boolean;
  };
}

export interface UploadProgress {
  uploadedBytes: number;
  totalBytes: number;
}

export interface UploadResult {
  projectId: string;
  assetId: string;
  filename: string;
  byteSize: number;
}

export interface ProjectSummary {
  id: string;
  displayName: string;
  folderId: string;
  folderName: string;
  primaryAssetId?: string;
  primaryExtension: string;
  favorite: boolean;
  addedAt: string;
  modifiedAt: string;
  lastOpenedAt?: string;
  missing: boolean;
  assetCount: number;
  coverPhotoId?: string;
  dimensionsMm?: [number, number, number];
}

export interface CompanionPage<T> {
  items: T[];
  total: number;
  nextCursor?: string;
}

export interface AssetMetadata {
  dimensionsMm?: [number, number, number];
  triangleCount?: number;
  objectCount?: number;
  materialNames?: string[];
  filamentGrams?: number;
  surfaceAreaMm2?: number;
  volumeMm3?: number;
  warning?: string;
  threeMf?: {
    slicer?: string;
    slicerVersion?: string;
    printer?: string;
    printProfile?: string;
    nozzleDiameterMm?: number;
    layerHeightMm?: number;
    plates: Array<{ index: number; name?: string; objectCount: number; objectNames: string[]; bedType?: string; filamentGrams?: number; printTimeSeconds?: number; materialNames: string[]; thumbnail: boolean }>;
  };
}

export interface CompanionAsset {
  id: string;
  filename: string;
  extension: string;
  byteSize: number;
  modifiedAt: string;
  parseStatus: string;
  metadata: AssetMetadata;
  missing: boolean;
  role: string;
}

export interface Tag {
  id: string;
  name: string;
  color: string;
  modelCount: number;
}

export interface ProjectDetail extends ProjectSummary {
  revision: string;
  notes: string;
  rootName: string;
  assets: CompanionAsset[];
  collectionIds: string[];
  tags: Tag[];
  estimate?: {
    id: string;
    modelId: string;
    assetId?: string;
    materialId?: string;
    plasticG: number;
    quantity: number;
    source: string;
    costPerPieceMinor?: number;
    batchCostMinor?: number;
  };
  webSource?: {
    provider: string;
    canonicalUrl: string;
    title: string;
    creator?: string;
    description?: string;
    license?: string;
    imageUrl?: string;
    filamentGrams?: number;
  };
}

export interface ProjectPhoto {
  id: string;
  modelId: string;
  originalFilename: string;
  mimeType: string;
  byteSize: number;
  width: number;
  height: number;
  caption: string;
  sortOrder: number;
  isCover: boolean;
  createdAt: string;
  updatedAt: string;
}

export interface ProjectCustomValue {
  fieldId: string;
  value: string | number | boolean | null;
  updatedAt: string;
}

export interface ProjectBomItem {
  id: string;
  modelId: string;
  kind: string;
  linkedAssetId?: string;
  name: string;
  quantity: number;
  unit: string;
  url?: string;
  notes: string;
  sortOrder: number;
  createdAt: string;
  updatedAt: string;
}

export interface AssemblyStep {
  id: string;
  modelId: string;
  title: string;
  instructions: string;
  sortOrder: number;
  bomItemIds: string[];
  photoIds: string[];
  createdAt: string;
  updatedAt: string;
}

export interface ProjectDocumentation {
  revision: string;
  photos: ProjectPhoto[];
  customValues: ProjectCustomValue[];
  bomItems: ProjectBomItem[];
  assemblySteps: AssemblyStep[];
}

export interface Collection {
  id: string;
  name: string;
  symbol: string;
  color: string;
  modelCount: number;
  smart: boolean;
  createdAt: string;
  rule?: SmartCollectionRule;
}

export interface SmartCollectionRule {
  version: number;
  matchMode: string;
  rules: Array<{ field: string; operator: string; value: unknown; customFieldId?: string }>;
  tagId?: string;
  format?: string;
  availability?: string;
}

export interface FolderSummary {
  id: string;
  parentId?: string;
  name: string;
  modelCount: number;
}

export interface ModelQuery {
  search?: string;
  folderId?: string;
  collectionId?: string;
  favorite?: boolean;
  recent?: boolean;
  format?: string;
  availability?: string;
  tagId?: string;
  dateField?: string;
  dateFrom?: string;
  dateTo?: string;
  sort?: "modified" | "added" | "opened" | "name";
}

export interface SavedSearch {
  id: string;
  name: string;
  query: ModelQuery;
  createdAt: string;
  updatedAt: string;
}

export interface Material {
  id: string;
  name: string;
  materialType: string;
  colorName?: string;
  colorHex?: string;
  spoolPriceMinor: number;
  currency: string;
  spoolWeightG: number;
  density?: number;
  updatedAt: string;
}

export interface RelatedProject {
  id: string;
  displayName: string;
  relativeFolder: string;
  primaryExtension: string;
  modifiedAt: string;
  relationship: "duplicate" | "geometry" | "version";
}

export interface OfflineManifest {
  schemaVersion: number;
  key: string;
  serverId: string;
  projectId: string;
  displayName: string;
  revision: string;
  syncedAt: string;
  byteSize: number;
  mediaCount: number;
}

export interface OfflineProjectSnapshot {
  manifest: OfflineManifest;
  detail: ProjectDetail;
  documentation: ProjectDocumentation;
  cover?: Blob;
  photoThumbnails: Array<{ photoId: string; blob: Blob }>;
  viewerAssetId?: string;
  viewerMesh?: ArrayBuffer;
}

export interface CustomFieldDefinition {
  id: string;
  name: string;
  fieldType: string;
  unit?: string;
  options: string[];
}
