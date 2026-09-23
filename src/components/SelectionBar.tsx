import { useState } from "react";
import { Check, Heart, Layers3, Merge, Tags, X } from "lucide-react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useAppStore } from "../app/store";
import { api } from "../lib/tauri/api";
import { t } from "../lib/i18n";

export function SelectionBar() {
  const queryClient = useQueryClient();
  const { selectedModelIds, clearModelSelection } = useAppStore();
  const { data: collections = [] } = useQuery({ queryKey: ["collections"], queryFn: api.collections });
  const { data: tags = [] } = useQuery({ queryKey: ["tags"], queryFn: api.tags });
  const [collectionId, setCollectionId] = useState("");
  const [tagId, setTagId] = useState("");
  if (!selectedModelIds.length) return null;
  const add = async (id: string) => {
    if (!id) return;
    await api.addToCollection(id, selectedModelIds);
    await queryClient.invalidateQueries({ queryKey: ["collections"] });
    clearModelSelection();
  };
  const addTag = async (id: string) => {
    if (!id) return;
    await api.addToTag(id, selectedModelIds);
    await Promise.all([queryClient.invalidateQueries({ queryKey: ["tags"] }), queryClient.invalidateQueries({ queryKey: ["models"] }), queryClient.invalidateQueries({ queryKey: ["collections"] })]);
    clearModelSelection();
  };
  const favorite = async () => {
    await api.setFavorite(selectedModelIds, true);
    await queryClient.invalidateQueries({ queryKey: ["models"] });
    clearModelSelection();
  };
  const bundle = async () => {
    if (selectedModelIds.length < 2 || !window.confirm(t("Bundle these projects? The first project you selected will be kept, and all files and organization metadata will be transferred to it."))) return;
    await api.mergeProjects(selectedModelIds[0], selectedModelIds.slice(1));
    await Promise.all([queryClient.invalidateQueries({ queryKey: ["models"] }), queryClient.invalidateQueries({ queryKey: ["collections"] }), queryClient.invalidateQueries({ queryKey: ["duplicate-groups"] })]);
    clearModelSelection();
  };
  return (
    <div className="selection-bar">
      <span className="selection-bar__count"><Check size={14} />{t("{count} selected", { count: selectedModelIds.length })}</span>
      <button onClick={favorite}><Heart size={15} /> {t("Favorite")}</button>
      {selectedModelIds.length > 1 && <button onClick={() => void bundle()}><Merge size={15} /> {t("Bundle as project")}</button>}
      <label><Layers3 size={15} /><select value={collectionId} onChange={(event) => { setCollectionId(event.target.value); add(event.target.value); }}><option value="">{t("Add to collection…")}</option>{collections.filter((collection) => !collection.smart).map((collection) => <option key={collection.id} value={collection.id}>{collection.name}</option>)}</select></label>
      {tags.length > 0 && <label><Tags size={15} /><select value={tagId} onChange={(event) => { setTagId(event.target.value); addTag(event.target.value); }}><option value="">{t("Add tag…")}</option>{tags.map((tag) => <option key={tag.id} value={tag.id}>{tag.name}</option>)}</select></label>}
      <button className="selection-bar__close" onClick={clearModelSelection} aria-label={t("Clear selection")}><X size={16} /></button>
    </div>
  );
}
