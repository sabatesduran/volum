import { Heart, MoreHorizontal } from "lucide-react";
import { useState } from "react";
import { api } from "../lib/tauri/api";
import { formatDate } from "../lib/format";
import { useAppStore } from "../app/store";
import type { ModelSummary } from "../types";
import { ModelThumbnail } from "./ModelThumbnail";

export function ModelCard({ model, context = "folder", onChanged }: { model: ModelSummary; context?: "folder" | "date" | "format"; onChanged?: () => void }) {
  const { selectModel, selectedModelIds, toggleModelSelection } = useAppStore();
  const selected = selectedModelIds.includes(model.id);
  const [menuOpen, setMenuOpen] = useState(false);
  const toggleFavorite = async (event: React.MouseEvent) => {
    event.stopPropagation();
    await api.toggleFavorite(model.id);
    onChanged?.();
  };
  return (
    <article
      className={`model-card ${selected ? "is-selected" : ""}`} tabIndex={0} draggable
      onDragStart={(event) => { const ids = selected ? selectedModelIds : [model.id]; event.dataTransfer.setData("application/x-volum-models", JSON.stringify(ids)); event.dataTransfer.effectAllowed = "copy"; }}
      onClick={(event) => { if (event.metaKey || event.ctrlKey || event.shiftKey) toggleModelSelection(model.id); else selectModel(model.id); }}
      onContextMenu={(event) => { event.preventDefault(); setMenuOpen(true); }}
      onKeyDown={(event) => { if (event.key === "Enter") selectModel(model.id); if (event.key === " ") { event.preventDefault(); toggleModelSelection(model.id); } }}
    >
      <div className="model-card__preview">
        <ModelThumbnail modelId={model.id} assetId={model.primaryAssetId} extension={model.primaryExtension} revision={model.modifiedAt} missing={model.missing} />
        <div className="model-card__actions">
          <button className={`round-action ${model.favorite ? "is-active" : ""}`} onClick={toggleFavorite} aria-label={model.favorite ? "Remove from favorites" : "Add to favorites"}><Heart size={16} fill={model.favorite ? "currentColor" : "none"} /></button>
          <button className="round-action" onClick={(event) => { event.stopPropagation(); setMenuOpen((value) => !value); }} aria-label="More actions"><MoreHorizontal size={17} /></button>
        </div>
        <span className="model-card__select">{selected ? "✓" : ""}</span>
        {menuOpen && <div className="card-menu" onClick={(event) => event.stopPropagation()} onMouseLeave={() => setMenuOpen(false)}><button onClick={() => selectModel(model.id)}>View details</button><button disabled={!model.primaryAssetId || model.missing} onClick={() => model.primaryAssetId && api.openAsset(model.primaryAssetId)}>Open in default app</button><button disabled={!model.primaryAssetId || model.missing} onClick={() => model.primaryAssetId && api.revealAsset(model.primaryAssetId)}>Reveal file</button><button onClick={async () => { await api.toggleFavorite(model.id); setMenuOpen(false); onChanged?.(); }}>{model.favorite ? "Remove from favorites" : "Add to favorites"}</button></div>}
      </div>
      <div className="model-card__copy">
        <h3>{model.displayName}</h3>
        <p>{context === "date" ? `Modified ${formatDate(model.modifiedAt)}` : context === "format" ? `${model.primaryExtension.toLocaleUpperCase()} · ${model.assetCount} ${model.assetCount === 1 ? "file" : "files"}` : model.folderName}</p>
      </div>
    </article>
  );
}
