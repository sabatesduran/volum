import { useEffect, useState } from "react";
import { api, onBackendEvent } from "../lib/tauri/api";
import { ModelArt } from "./ModelArt";

interface ThumbnailError {
  assetId: string;
  message: string;
}

export function ModelThumbnail({ modelId, assetId, extension, revision, missing }: { modelId: string; assetId?: string; extension: string; revision: string; missing: boolean }) {
  const [url, setUrl] = useState<string>();
  const [failed, setFailed] = useState(false);
  useEffect(() => {
    if (!assetId || modelId.startsWith("demo-") || missing || !["stl", "obj", "3mf"].includes(extension)) return;
    let active = true;
    let objectUrl = "";
    const unlisteners: Array<() => void> = [];
    const showCached = async () => {
      const bytes = await api.thumbnail(assetId);
      if (!active || !bytes?.length) return false;
      if (objectUrl) URL.revokeObjectURL(objectUrl);
      objectUrl = URL.createObjectURL(new Blob([new Uint8Array(bytes)], { type: "image/png" }));
      setUrl(objectUrl);
      setFailed(false);
      return true;
    };
    const setup = async () => {
      const listeners = await Promise.all([
        onBackendEvent<string>("preview-ready", (readyAssetId) => {
          if (readyAssetId === assetId) void showCached();
        }),
        onBackendEvent<ThumbnailError>("preview-error", (error) => {
          if (error.assetId === assetId && active) setFailed(true);
        })
      ]);
      if (!active) { listeners.forEach((unlisten) => unlisten()); return; }
      unlisteners.push(...listeners);
      if (!await showCached()) await api.requestThumbnail(assetId);
    };
    setUrl(undefined);
    setFailed(false);
    setup().catch(() => { if (active) setFailed(true); });
    return () => {
      active = false;
      unlisteners.forEach((unlisten) => unlisten());
      if (objectUrl) URL.revokeObjectURL(objectUrl);
    };
  }, [assetId, extension, missing, modelId, revision]);
  if (modelId.startsWith("demo-")) return <ModelArt modelId={modelId} extension={extension} missing={missing} />;
  if (url) return <div className="model-thumbnail"><img src={url} alt="" /><span>{extension}</span></div>;
  return <div className={`thumbnail-placeholder ${failed || missing ? "thumbnail-placeholder--failed" : "thumbnail-placeholder--loading"}`}><strong>{extension.toUpperCase()}</strong><small>{missing ? "Library offline" : failed ? "Preview unavailable" : "Making preview…"}</small></div>;
}
