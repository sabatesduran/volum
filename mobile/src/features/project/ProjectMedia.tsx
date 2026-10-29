import { Box, Layers3, LoaderCircle } from "lucide-react";
import { useEffect, useState } from "react";
import { requestCachedMedia } from "../../lib/mediaCache";
import type { Credential, ProjectPhoto } from "../../types";

export function ProjectImage({ credential, projectId, revision, className = "", offlineBlob }: { credential: Credential; projectId: string; revision?: string; className?: string; offlineBlob?: Blob }) {
  const [url, setUrl] = useState<string>();
  const [loading, setLoading] = useState(true);
  useEffect(() => {
    let active = true;
    let objectUrl = "";
    setLoading(true);
    if (offlineBlob) {
      objectUrl = URL.createObjectURL(offlineBlob);
      setUrl(objectUrl);
      setLoading(false);
      return () => { active = false; URL.revokeObjectURL(objectUrl); };
    }
    void requestCachedMedia(credential, `/api/v1/projects/${encodeURIComponent(projectId)}/cover`)
      .then((value) => { objectUrl = value; if (active) setUrl(value); else URL.revokeObjectURL(value); })
      .catch(() => undefined)
      .finally(() => { if (active) setLoading(false); });
    return () => { active = false; if (objectUrl) URL.revokeObjectURL(objectUrl); };
  }, [credential, offlineBlob, projectId, revision]);
  return <div className={`project-image ${className}`}>{url ? <img src={url} alt="" /> : <div className="project-image__fallback">{loading ? <LoaderCircle className="spin" size={24} /> : <><Box size={34} /><span>3D</span></>}</div>}</div>;
}

export function PhotoImage({ credential, photo, full = false, offlineBlob }: { credential: Credential; photo: ProjectPhoto; full?: boolean; offlineBlob?: Blob }) {
  const [url, setUrl] = useState<string>();
  useEffect(() => {
    let active = true;
    let objectUrl = "";
    if (offlineBlob) {
      objectUrl = URL.createObjectURL(offlineBlob);
      setUrl(objectUrl);
      return () => { active = false; URL.revokeObjectURL(objectUrl); };
    }
    void requestCachedMedia(credential, `/api/v1/photos/${encodeURIComponent(photo.id)}?thumbnail=${String(!full)}`, full ? photo.mimeType : "image/png")
      .then((value) => { objectUrl = value; if (active) setUrl(value); else URL.revokeObjectURL(value); })
      .catch(() => undefined);
    return () => { active = false; if (objectUrl) URL.revokeObjectURL(objectUrl); };
  }, [credential, full, offlineBlob, photo.id, photo.mimeType]);
  return <div className="photo-image">{url ? <img src={url} alt={photo.caption || photo.originalFilename} /> : <LoaderCircle className="spin" size={19} />}</div>;
}

export function PlateThumbnail({ credential, assetId, plateIndex, name, enabled = true }: { credential: Credential; assetId: string; plateIndex: number; name: string; enabled?: boolean }) {
  const [url, setUrl] = useState<string>();
  useEffect(() => {
    let objectUrl = "";
    let active = true;
    if (!enabled) return () => { active = false; };
    void requestCachedMedia(credential, `/api/v1/assets/${encodeURIComponent(assetId)}/plates/${plateIndex}/thumbnail`)
      .then((value) => { objectUrl = value; if (active) setUrl(value); else URL.revokeObjectURL(value); })
      .catch(() => undefined);
    return () => { active = false; if (objectUrl) URL.revokeObjectURL(objectUrl); };
  }, [assetId, credential, enabled, plateIndex]);
  return <div className="plate-thumbnail">{url ? <img src={url} alt={name} /> : <Layers3 />}</div>;
}
