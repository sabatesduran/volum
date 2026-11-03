import type {
  CompanionPage, Credential, OfflineManifest, OfflineProjectSnapshot, OfflineScope, ProjectDetail, ProjectDocumentation, ProjectSummary,
} from "../types";
import { localizedError, t } from "./i18n";
import { requestBinary, requestBlob, requestJson } from "./transport";

const DATABASE_NAME = "volum-companion-offline";
const STORE_NAME = "projects";
const SCOPES_STORE_NAME = "scopes";
const DATABASE_VERSION = 3;
const MANIFEST_VERSION = 2;
const MAX_PROJECT_BYTES = 250 * 1024 * 1024;
const STORAGE_RESERVE_BYTES = 50 * 1024 * 1024;

export interface OfflineDownloadProgress {
  completed: number;
  total: number;
  byteSize: number;
  label: string;
}

function key(serverId: string, projectId: string) {
  return `${serverId}:${projectId}`;
}

function emitOfflineChange() {
  if (typeof window !== "undefined") window.dispatchEvent(new CustomEvent("volum-offline-change"));
}

function database(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const request = indexedDB.open(DATABASE_NAME, DATABASE_VERSION);
    request.onupgradeneeded = (event) => {
      const store = request.result.objectStoreNames.contains(STORE_NAME)
        ? request.transaction!.objectStore(STORE_NAME)
        : request.result.createObjectStore(STORE_NAME, { keyPath: "manifest.key" });
      if (event.oldVersion > 0 && event.oldVersion < 2) {
        const cursor = store.openCursor();
        cursor.onsuccess = () => {
          if (!cursor.result) return;
          const snapshot = cursor.result.value as OfflineProjectSnapshot;
          snapshot.manifest.schemaVersion = MANIFEST_VERSION;
          cursor.result.update(snapshot);
          cursor.result.continue();
        };
      }
      if (!request.result.objectStoreNames.contains(SCOPES_STORE_NAME)) {
        request.result.createObjectStore(SCOPES_STORE_NAME, { keyPath: "key" });
      }
    };
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error ?? new Error("Offline storage is unavailable."));
  });
}

async function transaction<T>(mode: IDBTransactionMode, run: (store: IDBObjectStore) => IDBRequest<T>, storeName = STORE_NAME) {
  const db = await database();
  return new Promise<T>((resolve, reject) => {
    const tx = db.transaction(storeName, mode);
    const request = run(tx.objectStore(storeName));
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error ?? new Error("Offline storage could not be updated."));
    tx.oncomplete = () => db.close();
    tx.onerror = () => reject(tx.error ?? new Error("Offline storage could not be updated."));
  });
}

export async function listOfflineManifests(serverId: string): Promise<OfflineManifest[]> {
  const snapshots = await transaction<OfflineProjectSnapshot[]>("readonly", (store) => store.getAll());
  return snapshots
    .map((snapshot) => snapshot.manifest)
    .filter((manifest) => manifest.serverId === serverId)
    .sort((left, right) => right.syncedAt.localeCompare(left.syncedAt));
}

export async function listOfflineProjectSnapshots(serverId: string): Promise<OfflineProjectSnapshot[]> {
  const snapshots = await transaction<OfflineProjectSnapshot[]>("readonly", (store) => store.getAll());
  return snapshots
    .filter((snapshot) => snapshot.manifest.serverId === serverId)
    .sort((left, right) => right.manifest.syncedAt.localeCompare(left.manifest.syncedAt));
}

export function getOfflineProject(serverId: string, projectId: string): Promise<OfflineProjectSnapshot | undefined> {
  return transaction<OfflineProjectSnapshot | undefined>("readonly", (store) => store.get(key(serverId, projectId)));
}

export async function removeOfflineProject(serverId: string, projectId: string) {
  await transaction("readwrite", (store) => store.delete(key(serverId, projectId)));
  emitOfflineChange();
}

export async function clearOfflineProjects(serverId: string) {
  const manifests = await listOfflineManifests(serverId);
  await Promise.all(manifests.map((manifest) => transaction("readwrite", (store) => store.delete(manifest.key))));
  const scopes = await listOfflineScopes(serverId);
  await Promise.all(scopes.map((scope) => transaction("readwrite", (store) => store.delete(scope.key), SCOPES_STORE_NAME)));
  emitOfflineChange();
}

export async function listOfflineScopes(serverId: string): Promise<OfflineScope[]> {
  const scopes = await transaction<OfflineScope[]>("readonly", (store) => store.getAll(), SCOPES_STORE_NAME);
  return scopes.filter((scope) => scope.serverId === serverId).sort((left, right) => right.syncedAt.localeCompare(left.syncedAt));
}

export async function removeOfflineScope(key: string) {
  await transaction("readwrite", (store) => store.delete(key), SCOPES_STORE_NAME);
  emitOfflineChange();
}

export async function storeOfflineProject(snapshot: OfflineProjectSnapshot) {
  if (snapshot.manifest.key !== key(snapshot.manifest.serverId, snapshot.manifest.projectId)) {
    throw new Error("The offline snapshot identity is invalid.");
  }
  await transaction("readwrite", (store) => store.put(snapshot));
  emitOfflineChange();
}

export async function downloadOfflineProject(
  credential: Credential,
  projectId: string,
  onProgress: (progress: OfflineDownloadProgress) => void = () => undefined,
) {
  await ensureOfflineCapacity(1);
  try { await navigator.storage?.persist?.(); } catch { /* Persistence is optional. */ }
  const [detail, documentation] = await Promise.all([
    requestJson<ProjectDetail>(credential, `/api/v1/projects/${encodeURIComponent(projectId)}`),
    requestJson<ProjectDocumentation>(credential, `/api/v1/projects/${encodeURIComponent(projectId)}/documentation`),
  ]);
  const viewerAsset = detail.assets.find((asset) => asset.id === detail.primaryAssetId && !asset.missing)
    ?? detail.assets.find((asset) => ["stl", "3mf", "obj", "step", "stp"].includes(asset.extension.toLowerCase()) && !asset.missing);
  const total = 1 + documentation.photos.length + (viewerAsset ? 1 : 0);
  let completed = 0;
  let byteSize = 0;
  const update = (label: string, bytes: number) => {
    completed += 1;
    byteSize += bytes;
    if (byteSize > MAX_PROJECT_BYTES) throw new Error("This project needs more than the 250 MB offline limit.");
    onProgress({ completed, total, byteSize, label });
  };

  const cover = await requestBlob(credential, `/api/v1/projects/${encodeURIComponent(projectId)}/cover`, "image/png")
    .then((blob) => { update("Cover", blob.size); return blob; })
    .catch(() => { update("Cover unavailable", 0); return undefined; });
  const photoThumbnails: OfflineProjectSnapshot["photoThumbnails"] = [];
  for (const photo of documentation.photos) {
    const blob = await requestBlob(
      credential,
      `/api/v1/photos/${encodeURIComponent(photo.id)}?thumbnail=true`,
      "image/png",
    );
    photoThumbnails.push({ photoId: photo.id, blob });
    update(photo.caption || photo.originalFilename, blob.size);
  }
  let viewerMesh: ArrayBuffer | undefined;
  if (viewerAsset) {
    viewerMesh = await requestBinary(
      credential,
      `/api/v1/assets/${encodeURIComponent(viewerAsset.id)}/viewer-mesh`,
    );
    update("3D viewer", viewerMesh.byteLength);
  }
  const manifest: OfflineManifest = {
    schemaVersion: MANIFEST_VERSION,
    key: key(credential.serverId, projectId),
    serverId: credential.serverId,
    projectId,
    displayName: detail.displayName,
    revision: detail.revision,
    syncedAt: new Date().toISOString(),
    byteSize,
    mediaCount: (cover ? 1 : 0) + photoThumbnails.length + (viewerMesh ? 1 : 0),
  };
  const snapshot: OfflineProjectSnapshot = {
    manifest,
    detail,
    documentation,
    cover,
    photoThumbnails,
    viewerAssetId: viewerAsset?.id,
    viewerMesh,
  };
  await ensureOfflineCapacity(byteSize);
  try {
    await storeOfflineProject(snapshot);
  } catch (reason) {
    if (reason instanceof DOMException && reason.name === "QuotaExceededError") {
      throw new Error("This device does not have enough storage for the offline project.");
    }
    throw reason;
  }
  return snapshot;
}

export type OfflineScopeInput = Pick<OfflineScope, "serverId" | "kind" | "scopeId" | "label" | "params">;

export async function syncOfflineScope(
  credential: Credential,
  scopeInput: OfflineScopeInput,
  libraryRevision: string | undefined,
  onProgress: (progress: OfflineDownloadProgress) => void = () => undefined,
): Promise<OfflineScope> {
  if (credential.serverId !== scopeInput.serverId) throw new Error("The offline scope belongs to a different Volum Desktop.");
  const scopeKey = `${scopeInput.serverId}:${scopeInput.kind}:${scopeInput.scopeId}` as OfflineScope["key"];
  const existing = (await listOfflineScopes(scopeInput.serverId)).find((scope) => scope.key === scopeKey);
  const params = new URLSearchParams(scopeInput.params);
  params.delete("cursor");
  params.delete("limit");
  const projects: ProjectSummary[] = [];
  let cursor = "";
  try {
    do {
      const pageParams = new URLSearchParams(params);
      pageParams.set("limit", "100");
      if (cursor) pageParams.set("cursor", cursor);
      const page = await requestJson<CompanionPage<ProjectSummary>>(credential, `/api/v1/projects?${pageParams}`);
      projects.push(...page.items);
      cursor = page.nextCursor ?? "";
    } while (cursor);
  } catch (reason) {
    const scope: OfflineScope = {
      ...scopeInput,
      key: scopeKey,
      projectIds: existing?.projectIds ?? [],
      syncedAt: existing?.syncedAt ?? new Date().toISOString(),
      libraryRevision,
      error: localizedError(reason),
    };
    await transaction("readwrite", (store) => store.put(scope), SCOPES_STORE_NAME);
    emitOfflineChange();
    throw reason;
  }

  const failures: string[] = [];
  for (let index = 0; index < projects.length; index += 1) {
    const project = projects[index];
    try {
      await downloadOfflineProject(credential, project.id);
    } catch (reason) {
      failures.push(`${project.displayName}: ${localizedError(reason)}`);
    }
    onProgress({ completed: index + 1, total: projects.length, byteSize: 0, label: project.displayName });
  }
  const error = failures.length
    ? failures.length === 1
      ? t("One offline project could not be synced: {details}", { details: failures[0] })
      : t("{count} offline projects could not be synced: {details}", {
        count: failures.length,
        details: `${failures.slice(0, 3).join(" · ")}${failures.length > 3 ? ` · ${t("{count} more", { count: failures.length - 3 })}` : ""}`,
      })
    : undefined;
  const scope: OfflineScope = {
    ...scopeInput,
    key: scopeKey,
    projectIds: projects.map((project) => project.id),
    syncedAt: new Date().toISOString(),
    libraryRevision,
    error,
  };
  await transaction("readwrite", (store) => store.put(scope), SCOPES_STORE_NAME);
  emitOfflineChange();
  return scope;
}

export async function offlineStorageEstimate() {
  const estimate = await navigator.storage?.estimate?.();
  return { usage: estimate?.usage ?? 0, quota: estimate?.quota ?? 0 };
}

async function ensureOfflineCapacity(requiredBytes: number) {
  const estimate = await navigator.storage?.estimate?.();
  if (estimate?.quota == null || estimate.usage == null) return;
  const available = Math.max(0, estimate.quota - estimate.usage);
  if (available < requiredBytes + STORAGE_RESERVE_BYTES) {
    throw new Error("This device is low on storage. Free at least 50 MB before saving projects offline.");
  }
}
