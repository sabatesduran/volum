import { FolderPlus, LockKeyhole, Sparkles } from "lucide-react";
import { open } from "@tauri-apps/plugin-dialog";
import { api, isTauri } from "../lib/tauri/api";
import { Brand } from "./Brand";

export function Onboarding({ onComplete }: { onComplete: () => void }) {
  const choose = async () => {
    const path = isTauri() ? await open({ directory: true, multiple: false, title: "Choose your 3D model library" }) : "/Users/you/3D Models";
    if (!path || Array.isArray(path)) return;
    const root = await api.addRoot(path);
    onComplete();
    await api.startScan(root.id);
  };
  return (
    <div className="onboarding">
      <div className="onboarding__panel"><Brand /><div className="onboarding__art"><div className="contour contour--1" /><div className="contour contour--2" /><div className="contour contour--3" /><div className="onboarding__object"><span /><span /><span /></div></div></div>
      <div className="onboarding__copy"><div className="eyebrow">Welcome to Volum</div><h1>A beautiful home<br />for your 3D models.</h1><p>Choose one or more folders. Volum indexes them locally without moving or uploading a single file.</p><div className="onboarding__promises"><span><LockKeyhole size={17} /><span><strong>Private by design</strong><small>No account or cloud required</small></span></span><span><Sparkles size={17} /><span><strong>Your folders, enriched</strong><small>Automatic previews and metadata</small></span></span></div><button className="button button--primary button--large" onClick={choose}><FolderPlus size={18} /> Choose a model folder</button><small className="onboarding__footnote">Supports STL, 3MF, OBJ, STEP, and ZIP archives</small></div>
    </div>
  );
}
