import type { Credential } from "../types";
import { requestMediaResponse, type MediaResponse } from "./transport";

const DATABASE_NAME = "volum-companion-media-cache";
const STORE_NAME = "media";
const DATABASE_VERSION = 1;
const MAX_CACHE_BYTES = 96 * 1024 * 1024;
const MAX_CACHE_ENTRIES = 192;

interface CachedMediaEntry {
  key: string;
  serverId: string;
  path: string;
  etag: string;
  contentType: string;
  blob: Blob;
  byteSize: number;
  lastAccessedAt: number;
}

export type MediaFetcher = (
  credential: Credential,
  path: string,
  ifNoneMatch?: string,
) => Promise<MediaResponse>;

const inFlight = new Map<string, Promise<Blob>>();

function cacheKey(serverId: string, path: string) {
  return `${serverId}:${path}`;
}

function database(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const request = indexedDB.open(DATABASE_NAME, DATABASE_VERSION);
    request.onupgradeneeded = () => {
      if (!request.result.objectStoreNames.contains(STORE_NAME)) {
        const store = request.result.createObjectStore(STORE_NAME, { keyPath: "key" });
        store.createIndex("serverId", "serverId");
      }
    };
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error ?? new Error("Media cache is unavailable."));
  });
}

async function transaction<T>(mode: IDBTransactionMode, run: (store: IDBObjectStore) => IDBRequest<T>) {
  const db = await database();
  return new Promise<T>((resolve, reject) => {
    const tx = db.transaction(STORE_NAME, mode);
    const request = run(tx.objectStore(STORE_NAME));
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error ?? new Error("Media cache could not be updated."));
    tx.oncomplete = () => db.close();
    tx.onerror = () => reject(tx.error ?? new Error("Media cache could not be updated."));
  });
}

function getCachedMedia(serverId: string, path: string) {
  return transaction<CachedMediaEntry | undefined>("readonly", (store) => store.get(cacheKey(serverId, path)));
}

async function putCachedMedia(entry: CachedMediaEntry) {
  await transaction("readwrite", (store) => store.put(entry));
}

async function touchCachedMedia(entry: CachedMediaEntry) {
  await putCachedMedia({ ...entry, lastAccessedAt: Date.now() });
}

async function deleteEntries(keys: string[]) {
  if (!keys.length) return;
  const db = await database();
  await new Promise<void>((resolve, reject) => {
    const tx = db.transaction(STORE_NAME, "readwrite");
    const store = tx.objectStore(STORE_NAME);
    keys.forEach((key) => store.delete(key));
    tx.oncomplete = () => { db.close(); resolve(); };
    tx.onerror = () => { db.close(); reject(tx.error ?? new Error("Media cache could not be pruned.")); };
  });
}

export async function pruneMediaCache(
  maximumBytes = MAX_CACHE_BYTES,
  maximumEntries = MAX_CACHE_ENTRIES,
) {
  const entries = await transaction<CachedMediaEntry[]>("readonly", (store) => store.getAll());
  entries.sort((left, right) => left.lastAccessedAt - right.lastAccessedAt || left.key.localeCompare(right.key));
  let byteSize = entries.reduce((total, entry) => total + entry.byteSize, 0);
  let count = entries.length;
  const removals: string[] = [];
  for (const entry of entries) {
    if (byteSize <= maximumBytes && count <= maximumEntries) break;
    removals.push(entry.key);
    byteSize -= entry.byteSize;
    count -= 1;
  }
  await deleteEntries(removals);
  return { byteSize, count, removed: removals.length };
}

function isConnectionFailure(reason: unknown) {
  const message = reason instanceof Error ? reason.message : String(reason);
  return message.includes("did not respond")
    || message.includes("Could not reach Volum Desktop")
    || message.includes("connection to Volum Desktop failed");
}

async function loadCachedMedia(
  credential: Credential,
  path: string,
  fallbackContentType: string,
  fetcher: MediaFetcher,
) {
  const cached = await getCachedMedia(credential.serverId, path).catch(() => undefined);
  try {
    const response = await fetcher(credential, path, cached?.etag);
    if (response.status === 304) {
      if (!cached) throw new Error("Volum Desktop returned an invalid cached media response.");
      void touchCachedMedia(cached).catch(() => undefined);
      return cached.blob;
    }
    const blob = new Blob([response.bytes], { type: response.contentType || fallbackContentType });
    if (response.etag) {
      const entry: CachedMediaEntry = {
        key: cacheKey(credential.serverId, path),
        serverId: credential.serverId,
        path,
        etag: response.etag,
        contentType: blob.type,
        blob,
        byteSize: blob.size,
        lastAccessedAt: Date.now(),
      };
      try {
        await putCachedMedia(entry);
        await pruneMediaCache();
      } catch {
        // The transient cache is opportunistic and must never block media rendering.
      }
    }
    return blob;
  } catch (reason) {
    if (cached && isConnectionFailure(reason)) {
      void touchCachedMedia(cached).catch(() => undefined);
      return cached.blob;
    }
    throw reason;
  }
}

export function requestCachedBlob(
  credential: Credential,
  path: string,
  fallbackContentType = "application/octet-stream",
  fetcher: MediaFetcher = requestMediaResponse,
) {
  const key = cacheKey(credential.serverId, path);
  const active = inFlight.get(key);
  if (active) return active;
  const request = loadCachedMedia(credential, path, fallbackContentType, fetcher)
    .finally(() => inFlight.delete(key));
  inFlight.set(key, request);
  return request;
}

export async function requestCachedMedia(
  credential: Credential,
  path: string,
  fallbackContentType = "image/png",
) {
  return URL.createObjectURL(await requestCachedBlob(credential, path, fallbackContentType));
}

export async function requestCachedBinary(credential: Credential, path: string) {
  return (await requestCachedBlob(credential, path)).arrayBuffer();
}

export async function clearMediaCache(serverId: string) {
  const entries = await transaction<CachedMediaEntry[]>("readonly", (store) => store.getAll());
  await deleteEntries(entries.filter((entry) => entry.serverId === serverId).map((entry) => entry.key));
}

export async function mediaCacheStats(serverId?: string) {
  const entries = await transaction<CachedMediaEntry[]>("readonly", (store) => store.getAll());
  const selected = serverId ? entries.filter((entry) => entry.serverId === serverId) : entries;
  return {
    count: selected.length,
    byteSize: selected.reduce((total, entry) => total + entry.byteSize, 0),
  };
}
