import {
  Clock3, Copy, FolderTree, Globe2, Heart, LayoutGrid, LibraryBig, Plus, Settings2,
  Wrench, Gift, Sparkles, Box, type LucideIcon
} from "lucide-react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { api } from "../lib/tauri/api";
import { useAppStore } from "../app/store";
import { Brand } from "./Brand";
import type { ViewId } from "../types";

const mainItems: Array<[ViewId, string, LucideIcon]> = [
  ["library", "Library", LibraryBig],
  ["recent", "Recent", Clock3],
  ["favorites", "Favorites", Heart],
  ["duplicates", "Duplicates", Copy],
  ["web", "Web imports", Globe2]
];

const symbols: Record<string, LucideIcon> = { wrench: Wrench, gift: Gift, sparkles: Sparkles, box: Box };

export function Sidebar({ onNewCollection }: { onNewCollection: () => void }) {
  const { view, selectView, selectCollection, selectedCollectionId, sidebarCollapsed } = useAppStore();
  const queryClient = useQueryClient();
  const { data: collections = [] } = useQuery({ queryKey: ["collections"], queryFn: api.collections });
  const { data: duplicateStats } = useQuery({ queryKey: ["duplicate-stats"], queryFn: api.duplicateStats });
  const { data: webSources = [] } = useQuery({ queryKey: ["web-sources"], queryFn: api.webSources });
  const item = (id: ViewId, label: string, Icon: LucideIcon, count?: number) => (
    <button
      className={`sidebar__item ${view === id && !selectedCollectionId ? "is-active" : ""}`}
      onClick={() => selectView(id)} title={sidebarCollapsed ? label : undefined}
    >
      <Icon size={18} strokeWidth={1.8} />
      {!sidebarCollapsed && <span>{label}</span>}
      {!sidebarCollapsed && count != null && <span className="sidebar__count">{count}</span>}
    </button>
  );

  return (
    <aside className={`sidebar ${sidebarCollapsed ? "sidebar--collapsed" : ""}`}>
      <div className="sidebar__brand"><Brand compact={sidebarCollapsed} /></div>
      <nav className="sidebar__nav" aria-label="Main navigation">
        <div className="sidebar__group">
          {mainItems.map(([id, label, Icon]) => <div key={id}>{item(id, label, Icon, id === "duplicates" ? duplicateStats?.groups : id === "web" ? webSources.length : undefined)}</div>)}
        </div>
        <div className="sidebar__group">
          {item("folders", "Folders", FolderTree)}
          {item("collections", "Collections", LayoutGrid)}
        </div>
        {!sidebarCollapsed && collections.length > 0 && (
          <div className="sidebar__collections">
            <div className="sidebar__section-label">
              <span>Your collections</span>
              <button className="icon-button icon-button--tiny" onClick={onNewCollection} aria-label="New collection"><Plus size={15} /></button>
            </div>
            {collections.slice(0, 5).map((collection) => {
              const Icon = symbols[collection.symbol] ?? Box;
              return (
                <button
                  key={collection.id}
                  className={`sidebar__item sidebar__item--collection ${selectedCollectionId === collection.id ? "is-active" : ""}`}
                  onClick={() => selectCollection(collection.id)}
                  onDragOver={(event) => { if (!collection.smart && event.dataTransfer.types.includes("application/x-volum-models")) { event.preventDefault(); event.dataTransfer.dropEffect = "copy"; } }}
                  onDrop={collection.smart ? undefined : async (event) => { event.preventDefault(); try { const ids = JSON.parse(event.dataTransfer.getData("application/x-volum-models")) as string[]; await api.addToCollection(collection.id, ids); await queryClient.invalidateQueries({ queryKey: ["collections"] }); useAppStore.getState().clearModelSelection(); } catch { /* Ignore unrelated drags. */ } }}
                >
                  <span className="collection-dot" style={{ color: collection.color }}><Icon size={15} /></span>
                  <span>{collection.name}</span><span className="sidebar__count">{collection.modelCount}</span>
                </button>
              );
            })}
          </div>
        )}
      </nav>
      <div className="sidebar__bottom">{item("settings", "Settings", Settings2)}</div>
    </aside>
  );
}
