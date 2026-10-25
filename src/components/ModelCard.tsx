import { Heart, MoreHorizontal } from "lucide-react";
import { memo, useState } from "react";
import { api } from "../lib/tauri/api";
import { formatDate } from "../lib/format";
import { useAppStore } from "../app/store";
import type { ModelSummary } from "../types";
import { ModelThumbnail } from "./ModelThumbnail";
import { plural, t } from "../lib/i18n";

function ModelCardComponent({ model, context = "folder", onChanged, trashed = false }: { model: ModelSummary; context?: "folder" | "date" | "format"; onChanged?: () => void; trashed?: boolean }) {
  const selectModel = useAppStore((state) => state.selectModel);
  const toggleModelSelection = useAppStore((state) => state.toggleModelSelection);
  const selected = useAppStore((state) => state.selectedModelIds.includes(model.id));
  const [menuOpen, setMenuOpen] = useState(false);
  const toggleFavorite = async (event: React.MouseEvent) => {
    event.stopPropagation();
    await api.toggleFavorite(model.id);
    onChanged?.();
  };
  return (
    <article
      className={`model-card ${selected ? "is-selected" : ""}`} tabIndex={0} draggable
      onDragStart={(event) => { const selectedIds = useAppStore.getState().selectedModelIds; const ids = selected ? selectedIds : [model.id]; event.dataTransfer.setData("application/x-volum-models", JSON.stringify(ids)); event.dataTransfer.effectAllowed = "copy"; }}
      onClick={(event) => { if (trashed) setMenuOpen(true); else if (event.metaKey || event.ctrlKey || event.shiftKey) toggleModelSelection(model.id); else selectModel(model.id); }}
      onContextMenu={(event) => { event.preventDefault(); setMenuOpen(true); }}
      onKeyDown={(event) => { if (event.key === "Enter") trashed ? setMenuOpen(true) : selectModel(model.id); if (event.key === " ") { event.preventDefault(); if (trashed) setMenuOpen(true); else toggleModelSelection(model.id); } }}
    >
      <div className="model-card__preview">
        <ModelThumbnail modelId={model.id} assetId={model.primaryAssetId} coverPhotoId={model.coverPhotoId} extension={model.primaryExtension} revision={model.modifiedAt} missing={model.missing} />
        <div className="model-card__actions">
          {!trashed && <button className={`round-action ${model.favorite ? "is-active" : ""}`} onClick={toggleFavorite} aria-label={t(model.favorite ? "Remove from favorites" : "Add to favorites")}><Heart size={16} fill={model.favorite ? "currentColor" : "none"} /></button>}
          <button className="round-action" onClick={(event) => { event.stopPropagation(); setMenuOpen((value) => !value); }} aria-label={t("More actions")}><MoreHorizontal size={17} /></button>
        </div>
        <span className="model-card__select">{selected ? "✓" : ""}</span>
        {menuOpen && <div className="card-menu" onClick={(event) => event.stopPropagation()} onMouseLeave={() => setMenuOpen(false)}>{trashed ? <><button onClick={async () => { await api.restoreProject(model.id); setMenuOpen(false); onChanged?.(); }}>{t("Restore project")}</button><button className="is-danger" onClick={async () => { if (!window.confirm(t("Delete this project permanently? This cannot be undone."))) return; await api.deleteProjectPermanently(model.id); setMenuOpen(false); onChanged?.(); }}>{t("Delete permanently")}</button></> : <><button onClick={() => selectModel(model.id)}>{t("View details")}</button><button disabled={!model.primaryAssetId || model.missing} onClick={() => model.primaryAssetId && api.openAsset(model.primaryAssetId)}>{t("Open in default app")}</button><button disabled={!model.primaryAssetId || model.missing} onClick={() => model.primaryAssetId && api.revealAsset(model.primaryAssetId)}>{t("Reveal file")}</button><button onClick={async () => { await api.toggleFavorite(model.id); setMenuOpen(false); onChanged?.(); }}>{t(model.favorite ? "Remove from favorites" : "Add to favorites")}</button><button className="is-danger" onClick={async () => { if (!window.confirm(t("Move this project to Volum Trash?"))) return; await api.trashProject(model.id); setMenuOpen(false); onChanged?.(); }}>{t("Move to Trash")}</button></>}</div>}
      </div>
      <div className="model-card__copy">
        <h3>{model.displayName}</h3>
        <p>{context === "date" ? t("Modified {date}", { date: formatDate(model.modifiedAt) }) : context === "format" ? `${model.primaryExtension.toLocaleUpperCase()} · ${plural(model.assetCount, "{count} file", "{count} files")}` : model.folderName}</p>
      </div>
    </article>
  );
}

export const ModelCard = memo(ModelCardComponent);
