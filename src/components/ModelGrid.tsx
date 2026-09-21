import { useEffect, useMemo, useRef, useState } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { Box } from "lucide-react";
import { useAppStore } from "../app/store";
import type { ModelSummary } from "../types";
import { ModelCard } from "./ModelCard";
import { t } from "../lib/i18n";

interface ModelGridProps {
  models: ModelSummary[];
  loading?: boolean;
  loadingMore?: boolean;
  hasMore?: boolean;
  onLoadMore?: () => void;
  context?: "folder" | "date" | "format";
  onChanged?: () => void;
}

export function ModelGrid({ models, loading = false, loadingMore = false, hasMore = false, onLoadMore, context = "folder", onChanged }: ModelGridProps) {
  const density = useAppStore((state) => state.density);
  const parentRef = useRef<HTMLDivElement>(null);
  const [width, setWidth] = useState(1000);
  useEffect(() => {
    if (!parentRef.current) return;
    const observer = new ResizeObserver(([entry]) => setWidth(entry.contentRect.width));
    observer.observe(parentRef.current);
    return () => observer.disconnect();
  }, []);
  const minimum = density === "compact" ? 176 : density === "large" ? 320 : 238;
  const gap = density === "compact" ? 12 : 18;
  const columns = Math.max(1, Math.floor((width + gap) / (minimum + gap)));
  const cardWidth = (width - gap * (columns - 1)) / columns;
  const rowHeight = cardWidth * .75 + (density === "compact" ? 60 : 72) + gap;
  const modelRows = Math.ceil(models.length / columns);
  const virtualizer = useVirtualizer({
    count: modelRows + (hasMore ? 1 : 0),
    getScrollElement: () => parentRef.current,
    estimateSize: () => rowHeight,
    overscan: 3
  });
  useEffect(() => virtualizer.measure(), [columns, rowHeight, virtualizer]);
  const skeletons = useMemo(() => Array.from({ length: 8 }), []);
  const virtualRows = virtualizer.getVirtualItems();
  const lastVisibleRow = virtualRows.at(-1)?.index ?? -1;
  useEffect(() => {
    if (hasMore && !loadingMore && lastVisibleRow >= Math.max(0, modelRows - 2)) onLoadMore?.();
  }, [hasMore, lastVisibleRow, loadingMore, modelRows, onLoadMore]);

  if (loading && models.length === 0) return (
    <div className={`model-grid-static density-${density}`}>{skeletons.map((_, index) => <div className="model-card skeleton-card" key={index}><div className="model-card__preview skeleton" /><div className="skeleton skeleton--text" /><div className="skeleton skeleton--text-short" /></div>)}</div>
  );
  if (!loading && models.length === 0) return (
    <div className="empty-state"><span className="empty-state__icon"><Box size={24} /></span><h2>{t("No models here")}</h2><p>{t("Try another folder or clear your search filters.")}</p></div>
  );
  return (
    <div className="virtual-grid" ref={parentRef}>
      <div className="virtual-grid__sizer" style={{ height: virtualizer.getTotalSize() }}>
        {virtualRows.map((row) => row.index >= modelRows ? (
          <div className="virtual-grid__loader" key={row.key} style={{ transform: `translateY(${row.start}px)`, height: rowHeight }}><span className="spin-dot" />{t(loadingMore ? "Loading more models…" : "Scroll to load more")}</div>
        ) : (
          <div className="virtual-grid__row" key={row.key} style={{ transform: `translateY(${row.start}px)`, gridTemplateColumns: `repeat(${columns}, minmax(0, 1fr))`, gap }}>
            {models.slice(row.index * columns, row.index * columns + columns).map((model) => <ModelCard key={model.id} model={model} context={context} onChanged={onChanged} />)}
          </div>
        ))}
      </div>
    </div>
  );
}
