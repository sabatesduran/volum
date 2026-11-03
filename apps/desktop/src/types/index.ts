export type ViewId = "library" | "recent" | "favorites" | "duplicates" | "web" | "folders" | "collections" | "saved" | "trash" | "settings";
export type Density = "comfortable" | "compact" | "large";
export type Theme = "system" | "light" | "dark";
export type LanguagePreference = "system" | "en" | "ca" | "es";

export interface LibraryRoot {
  id: string;
  path: string;
  displayName: string;
  status: "online" | "offline" | "scanning" | "paused" | "error";
  lastScanAt?: string;
  modelCount: number;
  rootKind: "linked" | "managed";
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
  role: "source" | "printable" | "plate" | "reference" | "version" | "variant";
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
  bundleMode: "automatic" | "manual";
  storageMode: "linked" | "managed";
  coverPhotoId?: string;
  dimensionsMm?: [number, number, number];
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

export type CustomFieldType = "text" | "number" | "date" | "boolean" | "url" | "select";

export interface CustomFieldDefinition {
  id: string;
  name: string;
  fieldType: CustomFieldType;
  unit?: string;
  options: string[];
  sortOrder: number;
  createdAt: string;
  updatedAt: string;
}

export interface ProjectCustomValue {
  fieldId: string;
  value: string | number | boolean;
  updatedAt: string;
}

export interface ProjectBomItem {
  id: string;
  modelId: string;
  kind: "printed" | "purchased";
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
  photos: ProjectPhoto[];
  customValues: ProjectCustomValue[];
  bomItems: ProjectBomItem[];
  assemblySteps: AssemblyStep[];
}

export interface ProjectPackagePreview {
  formatVersion: number;
  appVersion: string;
  packageProjectId: string;
  packageExportId: string;
  projectName: string;
  exportedAt: string;
  fileCount: number;
  photoCount: number;
  bomItemCount: number;
  assemblyStepCount: number;
  totalBytes: number;
  alreadyImported: boolean;
}

export interface ManagedLibraryStatus {
  root?: LibraryRoot;
  projectsPath?: string;
  inboxPath?: string;
  trashPath?: string;
}

export interface LibraryAssetCandidate {
  id: string;
  projectId: string;
  projectName: string;
  filename: string;
  extension: string;
  byteSize: number;
  missing: boolean;
  storageMode: "linked" | "managed";
  rootName: string;
  relativePath: string;
}

export interface ConsolidationPreview {
  projectId: string;
  projectName: string;
  fileCount: number;
  totalBytes: number;
  sourceFolders: string[];
  managedFolderName: string;
  alreadyManaged: boolean;
}

export interface InboxImportResult {
  projectIds: string[];
  skipped: number;
  errors: string[];
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
  version?: number;
  matchMode?: "all" | "any";
  rules?: QueryRule[];
  tagId?: string;
  format?: string;
  availability?: "available" | "offline";
}

export type QueryRuleField = "text" | "folder" | "library" | "tag" | "format" | "availability" | "favorite" | "webSource" | "parseStatus" | "duplicate" | "added" | "modified" | "opened" | "fileCount" | "width" | "depth" | "height" | "custom";
export type QueryRuleOperator = "is" | "isNot" | "before" | "after" | "on" | "greaterThan" | "lessThan" | "atLeast" | "atMost";

export interface QueryRule {
  field: QueryRuleField;
  operator: QueryRuleOperator;
  value: string | number | boolean;
  customFieldId?: string;
}

export interface SavedSearch {
  id: string;
  name: string;
  query: ModelQuery;
  createdAt: string;
  updatedAt: string;
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
  relationship: "duplicate" | "geometry" | "version";
}

export interface DuplicateGroup {
  id: string;
  matchKey: string;
  matchKind: "exact" | "geometry";
  confidence: number;
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

export type BackupDestinationKind = "folder" | "webdav";
export type BackupSchedule = "manual" | "daily" | "weekly" | "monthly";

export interface BackupDestination {
  id: string;
  name: string;
  kind: BackupDestinationKind;
  location: string;
  username?: string;
  credentialSaved: boolean;
  schedule: BackupSchedule;
  retentionCount: number;
  enabled: boolean;
  lastAttemptAt?: string;
  lastSuccessAt?: string;
  lastError?: string;
  createdAt: string;
  updatedAt: string;
}

export interface BackupDestinationInput {
  id?: string;
  name: string;
  kind: BackupDestinationKind;
  location: string;
  username?: string;
  password?: string;
  schedule: BackupSchedule;
  retentionCount: number;
  enabled: boolean;
}

export interface BackupRun {
  id: string;
  destinationId?: string;
  destinationName: string;
  reason: "manual" | "scheduled";
  status: "running" | "complete" | "failed";
  filename: string;
  byteSize?: number;
  startedAt: string;
  completedAt?: string;
  error?: string;
}

export interface BackupArchive {
  key: string;
  filename: string;
  byteSize?: number;
  modifiedAt?: string;
}

export interface PreparedRestore {
  token: string;
  appVersion: string;
  createdAt: string;
  byteSize: number;
  counts: {
    libraries: number;
    models: number;
    collections: number;
    tags: number;
    webSources: number;
  };
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
  duplicateKind?: "exact" | "geometry" | "any";
  libraryId?: string;
  parseStatus?: string;
  hasWebSource?: boolean;
  minAssetCount?: number;
  maxAssetCount?: number;
  trashed?: boolean;
  sort?: ModelSort;
  cursor?: string;
  offset?: number;
  limit?: number;
}

export interface Page<T> {
  items: T[];
  total: number;
  nextOffset?: number;
  nextCursor?: string;
}

export interface PreviewPayload {
  extension: string;
  bytes: number[];
  sourceAssetId: string;
}

export interface CompanionDevice {
  id: string;
  name: string;
  platform: string;
  pairedAt: string;
  lastSeenAt?: string;
}

export interface CompanionStatus {
  enabled: boolean;
  running: boolean;
  serverId: string;
  tailnetIp?: string;
  dnsName?: string;
  baseUrl?: string;
  port: number;
  devices: CompanionDevice[];
  organizationWrites: boolean;
  photoUploads: boolean;
  networkMode: "automatic" | "custom";
  bindAddress?: string;
  publicUrl?: string;
  transport?: "tailscale-http" | "private-http" | "https";
  lastError?: string;
}

export interface CompanionPairingInfo {
  serverId: string;
  serverName: string;
  baseUrl: string;
  secret: string;
  expiresAt: string;
  transport: "tailscale-http" | "private-http" | "https";
  qrPayload: string;
}

export interface BackendConnectionStatus {
  mode: "local" | "remote";
  configured: boolean;
  online: boolean;
  serverId?: string;
  serverName?: string;
  baseUrl?: string;
  mirrorEnabled: boolean;
  mirrorPath?: string;
  mirrorMode?: "all" | "selected";
  lastSyncAt?: string;
  lastError?: string;
  localLibraryRetired?: boolean;
  migrationId?: string;
}

export interface LibraryMigrationStats {
  projects: number;
  files: number;
  photos: number;
  collections: number;
  tags: number;
  linkedProjects: number;
  managedProjects: number;
  missingFiles: number;
  totalBytes: number;
}

export interface LibraryMigrationPreflight {
  stats: LibraryMigrationStats;
  missing: Array<{ projectName: string; filename: string; source: string }>;
  unreadablePhotos: string[];
  canMigrate: boolean;
}

export interface PendingLibraryMigration {
  id: string;
  createdAt: string;
  stats: LibraryMigrationStats;
  serverId?: string;
}

export interface LibraryMigrationProgress {
  stage: "snapshot" | "hashing" | "transferring" | "verifying" | "activating";
  message: string;
  completedBytes: number;
  totalBytes: number;
  completedFiles: number;
  totalFiles: number;
}

export interface MirrorSyncReport {
  downloaded: number;
  uploaded: number;
  removed: number;
  conflicts: number;
  unchanged: number;
  totalAssets: number;
  cursor: number;
  completedAt: string;
}

export interface MirroredAsset {
  id: string;
  projectId: string;
  projectName: string;
  filename: string;
  byteSize: number;
  modifiedAt: string;
  mirrorPath: string;
}

export interface MirroredProject {
  projectId: string;
  projectName: string;
  assetCount: number;
  availableAssetCount: number;
  byteSize: number;
}

export interface RemoteMirrorSelection {
  projectIds: string[];
  excludedProjectIds: string[];
  folderIds: string[];
  collectionIds: string[];
}

export interface RemoteMirrorSettings extends RemoteMirrorSelection {
  enabled: boolean;
  path?: string;
  mode: "all" | "selected";
  lastSyncAt?: string;
  lastError?: string;
}
