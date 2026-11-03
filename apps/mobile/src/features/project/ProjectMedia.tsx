import { ImageOff, Layers3, LoaderCircle } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import noPreviewModel from "../../assets/no-preview-model.svg";
import { t } from "../../lib/i18n";
import { requestCachedMedia, requestFreshMedia } from "../../lib/mediaCache";
import type { Credential, ProjectPhoto } from "../../types";

function deferredRevoke(url: string) {
  if (typeof requestAnimationFrame === "function") requestAnimationFrame(() => URL.revokeObjectURL(url));
  else queueMicrotask(() => URL.revokeObjectURL(url));
}

function useMediaUrl({ credential, path, revision, fallbackContentType = "image/png", offlineBlob, enabled = true }: { credential: Credential; path: string; revision?: string; fallbackContentType?: string; offlineBlob?: Blob; enabled?: boolean }) {
  const [url, setUrl] = useState<string>();
  const [loading, setLoading] = useState(enabled);
  const currentUrl = useRef<string | undefined>(undefined);
  const generation = useRef(0);
  const sourceKey = `${offlineBlob ? "offline" : "remote"}:${credential.serverId}:${path}`;
  const sourceKeyRef = useRef(sourceKey);
  const retryKey = useRef("");

  const install = useCallback((next: string, expectedGeneration: number) => {
    if (generation.current !== expectedGeneration) {
      URL.revokeObjectURL(next);
      return;
    }
    const previous = currentUrl.current;
    currentUrl.current = next;
    setUrl(next);
    setLoading(false);
    if (previous && previous !== next) deferredRevoke(previous);
  }, []);

  useEffect(() => {
    const requestGeneration = ++generation.current;
    let active = true;
    if (sourceKeyRef.current !== sourceKey) {
      sourceKeyRef.current = sourceKey;
      retryKey.current = "";
      const previous = currentUrl.current;
      currentUrl.current = undefined;
      setUrl(undefined);
      if (previous) URL.revokeObjectURL(previous);
    }
    if (!enabled) {
      setLoading(false);
      return () => { active = false; };
    }
    setLoading(!currentUrl.current);
    const request = offlineBlob
      ? Promise.resolve(URL.createObjectURL(offlineBlob))
      : requestCachedMedia(credential, path, fallbackContentType);
    void request
      .then((next) => {
        if (!active) URL.revokeObjectURL(next);
        else install(next, requestGeneration);
      })
      .catch(() => { if (active && generation.current === requestGeneration) setLoading(false); });
    return () => { active = false; };
  }, [credential.baseUrl, credential.serverId, credential.token, enabled, fallbackContentType, install, offlineBlob, path, revision, sourceKey]);

  useEffect(() => () => {
    generation.current += 1;
    if (currentUrl.current) URL.revokeObjectURL(currentUrl.current);
    currentUrl.current = undefined;
  }, []);

  const retry = useCallback(() => {
    const version = `${sourceKey}:${revision ?? ""}`;
    const failed = currentUrl.current;
    currentUrl.current = undefined;
    setUrl(undefined);
    if (failed) URL.revokeObjectURL(failed);
    if (retryKey.current === version || !enabled) {
      setLoading(false);
      return;
    }
    retryKey.current = version;
    setLoading(true);
    const requestGeneration = ++generation.current;
    void requestFreshMedia(credential, path, fallbackContentType)
      .then((next) => install(next, requestGeneration))
      .catch(() => { if (generation.current === requestGeneration) setLoading(false); });
  }, [credential, enabled, fallbackContentType, install, path, revision, sourceKey]);

  return { url, loading, retry };
}

export function ProjectImage({ credential, projectId, revision, className = "", offlineBlob, detailFallback = false, canOpenViewer = false }: { credential: Credential; projectId: string; revision?: string; className?: string; offlineBlob?: Blob; detailFallback?: boolean; canOpenViewer?: boolean }) {
  const path = `/api/v1/projects/${encodeURIComponent(projectId)}/cover`;
  const { url, loading, retry } = useMediaUrl({ credential, path, revision, offlineBlob });
  return <div className={`project-image ${className}`}>{url ? <img src={url} alt="" onError={retry} /> : <div className={`project-image__fallback ${detailFallback ? "project-image__fallback--detail" : ""}`}>{loading ? <LoaderCircle className="spin" size={24} /> : detailFallback ? <><span className="project-image__fallback-art" aria-hidden="true"><img src={noPreviewModel} alt="" /></span><span className="project-image__fallback-copy"><strong>{t("No preview available")}</strong>{canOpenViewer && <small>{t("Open the 3D viewer to explore this file")}</small>}</span></> : <><ImageOff size={30} /><span>{t("No preview")}</span></>}</div>}</div>;
}

export function PhotoImage({ credential, photo, full = false, offlineBlob }: { credential: Credential; photo: ProjectPhoto; full?: boolean; offlineBlob?: Blob }) {
  const path = `/api/v1/photos/${encodeURIComponent(photo.id)}?thumbnail=${String(!full)}`;
  const { url, retry } = useMediaUrl({ credential, path, fallbackContentType: full ? photo.mimeType : "image/png", offlineBlob });
  return <div className="photo-image">{url ? <img src={url} alt={photo.caption || photo.originalFilename} onError={retry} /> : <LoaderCircle className="spin" size={19} />}</div>;
}

export function PlateThumbnail({ credential, assetId, plateIndex, name, enabled = true }: { credential: Credential; assetId: string; plateIndex: number; name: string; enabled?: boolean }) {
  const path = `/api/v1/assets/${encodeURIComponent(assetId)}/plates/${plateIndex}/thumbnail`;
  const { url, retry } = useMediaUrl({ credential, path, enabled });
  return <div className="plate-thumbnail">{url ? <img src={url} alt={name} onError={retry} /> : <Layers3 />}</div>;
}
