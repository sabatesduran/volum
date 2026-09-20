import { useEffect, useRef } from "react";
import { Box, ChevronDown, Copy } from "lucide-react";
import { useAppStore } from "../app/store";
import { formatBytes } from "../lib/format";
import type { DuplicateGroup } from "../types";
import { ModelThumbnail } from "./ModelThumbnail";

export function DuplicateStacks({ groups, loading, loadingMore, hasMore, onLoadMore }: { groups: DuplicateGroup[]; loading: boolean; loadingMore: boolean; hasMore: boolean; onLoadMore: () => void }) {
  const selectModel = useAppStore((state) => state.selectModel);
  const scrollRef = useRef<HTMLDivElement>(null);
  const sentinelRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const sentinel = sentinelRef.current;
    if (!sentinel || !hasMore) return;
    const observer = new IntersectionObserver(([entry]) => { if (entry.isIntersecting && !loadingMore) onLoadMore(); }, { root: scrollRef.current, rootMargin: "300px" });
    observer.observe(sentinel);
    return () => observer.disconnect();
  }, [hasMore, loadingMore, onLoadMore]);
  if (loading && !groups.length) return <div className="duplicate-grid duplicate-grid--loading">{Array.from({ length: 6 }, (_, index) => <div className="duplicate-stack skeleton-card" key={index}><div className="duplicate-stack__preview skeleton" /><div className="skeleton skeleton--text" /><div className="skeleton skeleton--text-short" /></div>)}</div>;
  if (!groups.length) return <div className="empty-state"><span className="empty-state__icon"><Box size={24} /></span><h2>No exact duplicates</h2><p>Volum compares file contents, not only filenames.</p></div>;
  return <div className="duplicate-grid-scroll" ref={scrollRef}><div className="duplicate-grid">{groups.map((group) => { const representative = group.models[0]; const folders = [...new Set(group.models.map((model) => model.relativeFolder || model.folderName))]; return <article className="duplicate-stack" key={group.id}><button className="duplicate-stack__preview" onClick={() => representative && selectModel(representative.id)}>{representative && <ModelThumbnail modelId={representative.id} assetId={representative.primaryAssetId} extension={representative.primaryExtension} revision={representative.modifiedAt} missing={representative.missing} />}<span className="duplicate-stack__layer duplicate-stack__layer--one" /><span className="duplicate-stack__layer duplicate-stack__layer--two" /><span className="duplicate-stack__count"><Copy size={12} />{group.modelCount}</span></button><div className="duplicate-stack__copy"><h3>{representative?.displayName ?? "Duplicate models"}</h3><p>{group.modelCount} exact copies · {formatBytes(group.byteSize)}</p><small>{folders.slice(0, 2).join(" · ")}{folders.length > 2 ? ` +${folders.length - 2}` : ""}</small></div><details className="duplicate-stack__review"><summary>Review {group.modelCount} files <ChevronDown size={13} /></summary><div>{group.models.map((model) => <button key={model.id} onClick={() => selectModel(model.id)}><span className="file-icon">{model.primaryExtension.toUpperCase()}</span><span><strong>{model.displayName}</strong><small>{model.relativeFolder || model.folderName}</small></span></button>)}</div></details></article>; })}</div>{hasMore && <div className="duplicate-grid__sentinel" ref={sentinelRef}>{loadingMore ? "Loading more stacks…" : "Scroll to load more"}</div>}</div>;
}
