import { AppWindow } from "lucide-react";
import { useEffect, useState } from "react";
import type { SlicerApplication } from "../types";

export function SlicerIcon({ application, size = 24 }: { application?: Pick<SlicerApplication, "name" | "brandColor" | "iconPng">; size?: number }) {
  const [source, setSource] = useState<string>();
  useEffect(() => {
    if (!application?.iconPng?.length) { setSource(undefined); return; }
    const url = URL.createObjectURL(new Blob([new Uint8Array(application.iconPng)], { type: "image/png" }));
    setSource(url);
    return () => URL.revokeObjectURL(url);
  }, [application?.iconPng]);
  if (source) return <img className="slicer-icon slicer-icon--image" src={source} alt="" style={{ width: size, height: size }} />;
  return <span className="slicer-icon slicer-icon--fallback" style={{ width: size, height: size, background: application?.brandColor ?? "#6f6d67" }}><AppWindow size={Math.max(12, size * .55)} /></span>;
}
