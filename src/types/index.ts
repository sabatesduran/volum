export type ViewId = "library" | "recent" | "favorites" | "duplicates" | "web" | "folders" | "collections" | "settings";
export type Density = "comfortable" | "compact" | "large";
export type Theme = "system" | "light" | "dark";

export interface LibraryRoot {
  id: string;
  path: string;
  displayName: string;
  status: "online" | "offline" | "scanning" | "paused" | "error";
  lastScanAt?: string;
  modelCount: number;
}

export interface Folder {
  id: string;
  rootId: string;
  parentId?: string;
  relativePath: string;
  name: string;
  modelCount: number;
}

export interface Asset {
  id: string;
  filename: string;
  extension: string;
  relativePath: string;
  byteSize: number;
  modifiedAt: string;
  parseStatus: string;
  metadata: AssetMetadata;
  missing: boolean;
}

export interface AssetMetadata {
  dimensionsMm?: [number, number, number];
  triangleCount?: number;
  objectCount?: number;
  materialNames?: string[];
  filamentGrams?: number;
  warning?: string;
  threeMf?: ThreeMfMetadata;
}

export interface ThreeMfMetadata {
  slicer?: string;
  slicerVersion?: string;
  printer?: string;
  printProfile?: string;
  nozzleDiameterMm?: number;
  layerHeightMm?: number;
  plates?: ThreeMfPlate[];
}

export interface ThreeMfPlate {
  index: number;
  name?: string;
  objectCount: number;
  objectNames?: string[];
  bedType?: string;
  filamentGrams?: number;
  printTimeSeconds?: number;
  materialNames?: string[];
  thumbnail: boolean;
}

export interface ModelSummary {
  id: string;
  displayName: string;
  folderId: string;
  folderName: string;
  relativeFolder: string;
  primaryAssetId?: string;
  primaryExtension: string;
  favorite: boolean;
  addedAt: string;
  modifiedAt: string;
  lastOpenedAt?: string;
  missing: boolean;
  assetCount: number;
  dimensionsMm?: [number, number, number];
}

export interface ModelDetail extends ModelSummary {
  notes: string;
  rootId: string;
  rootName: string;
  rootPath: string;
  assets: Asset[];
  collectionIds: string[];
  tags: Tag[];
  estimate?: CostEstimate;
  webSource?: WebSource;
}

export interface WebSourcePreview {
  provider: "makerworld" | "printables";
  remoteId?: string;
  canonicalUrl: string;
  title: string;
  creator?: string;
  description?: string;
  license?: string;
  imageUrl?: string;
  filamentGrams?: number;
}

export interface WebSource extends WebSourcePreview {
  id: string;
  status: "saved" | "importing" | "imported";
  rootId?: string;
  relativePath?: string;
  modelId?: string;
  fetchedAt: string;
  createdAt: string;
  updatedAt: string;
}

export interface SmartCollectionRule {
  tagId?: string;
  format?: string;
  availability?: "available" | "offline";
}

export interface Collection {
  id: string;
  name: string;
  symbol: string;
  color: string;
  modelCount: number;
  createdAt: string;
  smart: boolean;
  rule?: SmartCollectionRule;
}

export interface Tag {
  id: string;
  name: string;
  color: string;
  modelCount: number;
}

export interface RelatedModel {
  id: string;
  displayName: string;
  relativeFolder: string;
  primaryExtension: string;
  modifiedAt: string;
  relationship: "duplicate" | "version";
}

export interface DuplicateGroup {
  id: string;
  modelCount: number;
  byteSize: number;
  models: ModelSummary[];
}

export interface DuplicateStats {
  groups: number;
  models: number;
  redundantCopies: number;
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

export interface ConfiguredSlicer {
  id: string;
  name: string;
  path: string;
}

export interface SlicerApplication extends ConfiguredSlicer {
  installed: boolean;
  custom: boolean;
  brandColor: string;
  iconPng?: number[];
}

export interface SlicerConfig {
  enabledIds: string[];
  defaultId?: string;
  customApps: ConfiguredSlicer[];
}

export interface CostEstimate {
  id: string;
  modelId: string;
  assetId?: string;
  materialId?: string;
  plasticG: number;
  quantity: number;
  source: "file" | "web" | "volume" | "manual";
  costPerPieceMinor?: number;
  batchCostMinor?: number;
}

export interface ScanStatus {
  rootId: string;
  state: "idle" | "scanning" | "paused" | "complete" | "error";
  discovered: number;
  processed: number;
  errors: number;
  currentPath?: string;
  message?: string;
}

export type ModelSort = "name" | "added" | "modified" | "opened";

export interface ModelQuery {
  search?: string;
  folderId?: string;
  collectionId?: string;
  favorite?: boolean;
  recent?: boolean;
  format?: string;
  availability?: "available" | "offline";
  tagId?: string;
  dateField?: "added" | "modified";
  dateFrom?: string;
  dateTo?: string;
  duplicates?: boolean;
  sort?: ModelSort;
  offset?: number;
  limit?: number;
}

export interface Page<T> {
  items: T[];
  total: number;
  nextOffset?: number;
}

export interface PreviewPayload {
  extension: string;
  bytes: number[];
  sourceAssetId: string;
}
