import type {
  Credential, OfflineManifest, OfflineProjectSnapshot, ProjectDetail, ProjectDocumentation,
} from "../types";
import { requestBinary, requestBlob, requestJson } from "./transport";

const DATABASE_NAME = "volum-companion-offline";
const STORE_NAME = "projects";
const DATABASE_VERSION = 2;
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
          snapshot.manifest.schemaVersion = 2;
          cursor.result.update(snapshot);
          cursor.result.continue();
        };
      }
    };
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error ?? new Error("Offline storage is unavailable."));
  });
}

async function transaction<T>(mode: IDBTransactionMode, run: (store: IDBObjectStore) => IDBRequest<T>) {
  const db = await database();
  return new Promise<T>((resolve, reject) => {
    const tx = db.transaction(STORE_NAME, mode);
    const request = run(tx.objectStore(STORE_NAME));
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
    schemaVersion: DATABASE_VERSION,
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
