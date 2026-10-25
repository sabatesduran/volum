import { useEffect, useRef, useState } from "react";
import { api } from "../lib/tauri/api";
import { ModelArt } from "./ModelArt";
import { t } from "../lib/i18n";
import { cachedObjectUrl, subscribeThumbnailEvents } from "../lib/mediaCache";

export function ModelThumbnail({ modelId, assetId, coverPhotoId, extension, revision, missing }: { modelId: string; assetId?: string; coverPhotoId?: string; extension: string; revision: string; missing: boolean }) {
  const [url, setUrl] = useState<string>();
  const [failed, setFailed] = useState(false);
  const [visible, setVisible] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const element = rootRef.current;
    if (!element || typeof IntersectionObserver === "undefined") { setVisible(true); return; }
    const observer = new IntersectionObserver((entries) => {
      if (entries.some((entry) => entry.isIntersecting)) {
        setVisible(true);
        observer.disconnect();
      }
    }, { rootMargin: "600px" });
    observer.observe(element);
    return () => observer.disconnect();
  }, [assetId, coverPhotoId]);
  useEffect(() => {
    setUrl(undefined);
    setFailed(false);
    if (!visible || (!coverPhotoId && (!assetId || modelId.startsWith("demo-") || missing || !["stl", "obj", "3mf", "step", "stp"].includes(extension)))) return;
    let active = true;
    const showCached = async () => {
      if (coverPhotoId) {
        const source = await cachedObjectUrl(`photo:${coverPhotoId}:${revision}`, "image/png", () => api.projectPhoto(coverPhotoId, true));
        if (!active || !source) return false;
        setUrl(source);
        setFailed(false);
        return true;
      }
      if (!assetId) return false;
      const source = await cachedObjectUrl(`thumbnail:${assetId}:${revision}`, "image/png", () => api.thumbnail(assetId));
      if (!active || !source) return false;
      setUrl(source);
      setFailed(false);
      return true;
    };
    const setup = async () => {
      if (coverPhotoId) {
        await showCached();
        return;
      }
      if (!assetId) return;
      const unsubscribe = subscribeThumbnailEvents(
        assetId,
        () => { if (active) void showCached(); },
        () => { if (active) setFailed(true); }
      );
      if (!active) { unsubscribe(); return; }
      cleanup = unsubscribe;
      if (!await showCached()) await api.requestThumbnail(assetId);
    };
    let cleanup: () => void = () => undefined;
    setup().catch(() => { if (active) setFailed(true); });
    return () => {
      active = false;
      cleanup();
    };
  }, [assetId, coverPhotoId, extension, missing, modelId, revision, visible]);
  if (modelId.startsWith("demo-") && !coverPhotoId) return <ModelArt modelId={modelId} extension={extension} missing={missing} />;
  if (url) return <div ref={rootRef} className={`model-thumbnail ${coverPhotoId ? "model-thumbnail--photo" : ""}`}><img src={url} alt="" />{!coverPhotoId && <span>{extension}</span>}</div>;
  return <div ref={rootRef} className={`thumbnail-placeholder ${failed || missing ? "thumbnail-placeholder--failed" : "thumbnail-placeholder--loading"}`}><strong>{extension.toUpperCase()}</strong><small>{t(missing ? "Library offline" : failed ? "Preview unavailable" : "Making preview…")}</small></div>;
}
