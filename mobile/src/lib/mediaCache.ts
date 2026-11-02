import type { Credential } from "../types";
import { isConnectionError, requestMediaResponse, type MediaResponse } from "./transport";

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
  bytes?: ArrayBuffer;
  blob?: Blob;
  byteSize: number;
  lastAccessedAt: number;
}

export type MediaFetcher = (
  credential: Credential,
  path: string,
  ifNoneMatch?: string,
) => Promise<MediaResponse>;

const inFlight = new Map<string, Promise<Blob>>();
const cacheGenerations = new Map<string, number>();
const mutationQueues = new Map<string, Promise<void>>();

function cacheGeneration(serverId: string) {
  return cacheGenerations.get(serverId) ?? 0;
}

function queueCacheMutation(
  serverId: string,
  expectedGeneration: number | undefined,
  mutation: () => Promise<void>,
) {
  const previous = mutationQueues.get(serverId)?.catch(() => undefined) ?? Promise.resolve();
  const queued = previous.then(async () => {
    if (expectedGeneration !== undefined && cacheGeneration(serverId) !== expectedGeneration) return;
    await mutation();
  });
  mutationQueues.set(serverId, queued);
  return queued.finally(() => {
    if (mutationQueues.get(serverId) === queued) mutationQueues.delete(serverId);
  });
}

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
    let result!: T;
    request.onsuccess = () => { result = request.result; };
    request.onerror = () => reject(request.error ?? new Error("Media cache could not be updated."));
    tx.oncomplete = () => { db.close(); resolve(result); };
    tx.onerror = () => { db.close(); reject(tx.error ?? new Error("Media cache could not be updated.")); };
    tx.onabort = () => { db.close(); reject(tx.error ?? new Error("Media cache could not be updated.")); };
  });
}

function getCachedMedia(serverId: string, path: string) {
  return transaction<CachedMediaEntry | undefined>("readonly", (store) => store.get(cacheKey(serverId, path)));
}

async function putCachedMedia(entry: CachedMediaEntry, generation: number) {
  await queueCacheMutation(entry.serverId, generation, async () => {
    await transaction("readwrite", (store) => store.put(entry));
  });
}

async function touchCachedMedia(entry: CachedMediaEntry, generation: number) {
  await queueCacheMutation(entry.serverId, generation, async () => {
    const current = await getCachedMedia(entry.serverId, entry.path);
    if (!current || current.etag !== entry.etag) return;
    await transaction("readwrite", (store) => store.put({ ...current, lastAccessedAt: Date.now() }));
  });
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
  const grouped = new Map<string, string[]>();
  for (const key of removals) {
    const entry = entries.find((candidate) => candidate.key === key);
    if (entry) grouped.set(entry.serverId, [...(grouped.get(entry.serverId) ?? []), key]);
  }
  await Promise.all([...grouped].map(([serverId, keys]) =>
    queueCacheMutation(serverId, cacheGeneration(serverId), () => deleteEntries(keys))
  ));
  return { byteSize, count, removed: removals.length };
}

function isConnectionFailure(reason: unknown) {
  return isConnectionError(reason);
}

async function cachedEntryBlob(entry: CachedMediaEntry, fallbackContentType: string, generation: number) {
  if (entry.bytes instanceof ArrayBuffer) {
    return new Blob([entry.bytes], { type: entry.contentType || fallbackContentType });
  }
  if (entry.blob instanceof Blob) {
    const bytes = await entry.blob.arrayBuffer();
    await queueCacheMutation(entry.serverId, generation, async () => {
      const current = await getCachedMedia(entry.serverId, entry.path);
      if (!current || current.etag !== entry.etag || current.bytes instanceof ArrayBuffer) return;
      await transaction("readwrite", (store) => store.put({
          ...current,
          bytes,
          blob: undefined,
          byteSize: bytes.byteLength,
          lastAccessedAt: Date.now(),
        }));
    }).catch(() => undefined);
    return new Blob([bytes], { type: entry.contentType || entry.blob.type || fallbackContentType });
  }
  throw new Error("Cached media is unreadable.");
}

function responseEntry(credential: Credential, path: string, response: MediaResponse, fallbackContentType: string): CachedMediaEntry {
  const bytes = response.bytes.slice(0);
  return {
    key: cacheKey(credential.serverId, path),
    serverId: credential.serverId,
    path,
    etag: response.etag ?? "",
    contentType: response.contentType || fallbackContentType,
    bytes,
    byteSize: bytes.byteLength,
    lastAccessedAt: Date.now(),
  };
}

async function loadCachedMedia(
  credential: Credential,
  path: string,
  fallbackContentType: string,
  fetcher: MediaFetcher,
) {
  const generation = cacheGeneration(credential.serverId);
  const cached = await getCachedMedia(credential.serverId, path).catch(() => undefined);
  try {
    const response = await fetcher(credential, path, cached?.etag);
    if (response.status === 304) {
      if (!cached) throw new Error("Volum Desktop returned an invalid cached media response.");
      await touchCachedMedia(cached, generation).catch(() => undefined);
      return cachedEntryBlob(cached, fallbackContentType, generation);
    }
    const blob = new Blob([response.bytes], { type: response.contentType || fallbackContentType });
    if (response.etag) {
      const entry = responseEntry(credential, path, response, fallbackContentType);
      try {
        await putCachedMedia(entry, generation);
        await pruneMediaCache();
      } catch {
        // The transient cache is opportunistic and must never block media rendering.
      }
    }
    return blob;
  } catch (reason) {
    if (cached && isConnectionFailure(reason)) {
      await touchCachedMedia(cached, generation).catch(() => undefined);
      return cachedEntryBlob(cached, fallbackContentType, generation);
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
  let request: Promise<Blob>;
  request = loadCachedMedia(credential, path, fallbackContentType, fetcher)
    .finally(() => {
      if (inFlight.get(key) === request) inFlight.delete(key);
    });
  inFlight.set(key, request);
  return request;
}

export async function requestCachedMedia(
  credential: Credential,
  path: string,
  fallbackContentType = "image/png",
  fetcher: MediaFetcher = requestMediaResponse,
) {
  const generation = cacheGeneration(credential.serverId);
  const cached = await getCachedMedia(credential.serverId, path).catch(() => undefined);
  if (cached) {
    try {
      const blob = await cachedEntryBlob(cached, fallbackContentType, generation);
      void requestCachedBlob(credential, path, fallbackContentType, fetcher).catch(() => undefined);
      return URL.createObjectURL(blob);
    } catch {
      await queueCacheMutation(credential.serverId, generation, async () => {
        const current = await getCachedMedia(credential.serverId, path);
        if (current?.etag === cached.etag) await deleteEntries([cached.key]);
      }).catch(() => undefined);
    }
  }
  return URL.createObjectURL(await requestCachedBlob(credential, path, fallbackContentType, fetcher));
}

export async function requestFreshMedia(
  credential: Credential,
  path: string,
  fallbackContentType = "image/png",
  fetcher: MediaFetcher = requestMediaResponse,
) {
  const generation = cacheGeneration(credential.serverId);
  await queueCacheMutation(credential.serverId, generation, () => deleteEntries([cacheKey(credential.serverId, path)])).catch(() => undefined);
  const response = await fetcher(credential, path);
  if (response.status !== 200) throw new Error("Volum Desktop returned an invalid fresh media response.");
  const entry = responseEntry(credential, path, response, fallbackContentType);
  if (response.etag) {
    try {
      await putCachedMedia(entry, generation);
      await pruneMediaCache();
    } catch {
      // A fresh response can still be displayed when the opportunistic cache is unavailable.
    }
  }
  return URL.createObjectURL(new Blob([response.bytes], { type: entry.contentType }));
}

export async function requestCachedBinary(credential: Credential, path: string) {
  return (await requestCachedBlob(credential, path)).arrayBuffer();
}

export async function clearMediaCache(serverId: string) {
  cacheGenerations.set(serverId, cacheGeneration(serverId) + 1);
  const prefix = `${serverId}:`;
  for (const key of inFlight.keys()) {
    if (key.startsWith(prefix)) inFlight.delete(key);
  }
  await queueCacheMutation(serverId, undefined, async () => {
    const entries = await transaction<CachedMediaEntry[]>("readonly", (store) => store.getAll());
    await deleteEntries(entries.filter((entry) => entry.serverId === serverId).map((entry) => entry.key));
  });
}

export async function mediaCacheStats(serverId?: string) {
  const entries = await transaction<CachedMediaEntry[]>("readonly", (store) => store.getAll());
  const selected = serverId ? entries.filter((entry) => entry.serverId === serverId) : entries;
  return {
    count: selected.length,
    byteSize: selected.reduce((total, entry) => total + entry.byteSize, 0),
  };
}
