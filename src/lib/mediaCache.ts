import { api, onBackendEvent } from "./tauri/api";

const MAX_OBJECT_URLS = 128;
const objectUrls = new Map<string, string>();
const pendingUrls = new Map<string, Promise<string | undefined>>();

export async function cachedObjectUrl(
  key: string,
  mimeType: string,
  loader: () => Promise<ArrayBuffer>
): Promise<string | undefined> {
  const cached = objectUrls.get(key);
  if (cached) {
    objectUrls.delete(key);
    objectUrls.set(key, cached);
    return cached;
  }
  const pending = pendingUrls.get(key);
  if (pending) return pending;
  const request = loader().then((buffer) => {
    if (!buffer.byteLength) return undefined;
    const url = URL.createObjectURL(new Blob([buffer], { type: mimeType }));
    objectUrls.set(key, url);
    while (objectUrls.size > MAX_OBJECT_URLS) {
      const oldest = objectUrls.entries().next().value as [string, string] | undefined;
      if (!oldest) break;
      objectUrls.delete(oldest[0]);
      URL.revokeObjectURL(oldest[1]);
    }
    return url;
  }).finally(() => pendingUrls.delete(key));
  pendingUrls.set(key, request);
  return request;
}

interface ThumbnailError {
  assetId: string;
  message: string;
}

type ReadyHandler = () => void;
type ErrorHandler = (message: string) => void;

const readyHandlers = new Map<string, Set<ReadyHandler>>();
const errorHandlers = new Map<string, Set<ErrorHandler>>();
let backendListeners: Promise<void> | undefined;

function ensureThumbnailListeners() {
  if (backendListeners) return;
  backendListeners = Promise.all([
    onBackendEvent<string>("preview-ready", (assetId) => {
      readyHandlers.get(assetId)?.forEach((handler) => handler());
    }),
    onBackendEvent<ThumbnailError>("preview-error", (error) => {
      errorHandlers.get(error.assetId)?.forEach((handler) => handler(error.message));
    })
  ]).then(() => undefined).catch(() => {
    backendListeners = undefined;
  });
}

export function subscribeThumbnailEvents(
  assetId: string,
  onReady: ReadyHandler,
  onError: ErrorHandler
) {
  ensureThumbnailListeners();
  const ready = readyHandlers.get(assetId) ?? new Set<ReadyHandler>();
  const errors = errorHandlers.get(assetId) ?? new Set<ErrorHandler>();
  ready.add(onReady);
  errors.add(onError);
  readyHandlers.set(assetId, ready);
  errorHandlers.set(assetId, errors);
  return () => {
    ready.delete(onReady);
    errors.delete(onError);
    if (!ready.size) readyHandlers.delete(assetId);
    if (!errors.size) errorHandlers.delete(assetId);
    if (!ready.size && !errors.size) void api.cancelThumbnail(assetId);
  };
}
