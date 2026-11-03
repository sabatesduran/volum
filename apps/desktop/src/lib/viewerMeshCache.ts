import { api } from "./tauri/api";

const viewerPayloads = new Map<string, Promise<ArrayBuffer>>();

export function cachedViewerPayload(assetId: string, plateIndex?: number, revision = "") {
  const key = `${assetId}:${plateIndex ?? "all"}:${revision}`;
  const cached = viewerPayloads.get(key);
  if (cached) {
    viewerPayloads.delete(key);
    viewerPayloads.set(key, cached);
    return cached;
  }
  const request = api.viewerMesh(assetId, plateIndex).catch((error) => {
    viewerPayloads.delete(key);
    throw error;
  });
  viewerPayloads.set(key, request);
  while (viewerPayloads.size > 4) viewerPayloads.delete(viewerPayloads.keys().next().value!);
  return request;
}

export function clearCachedViewerPayloads() {
  viewerPayloads.clear();
}
