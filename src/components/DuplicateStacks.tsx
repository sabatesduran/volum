import { useEffect, useMemo, useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { Box, Check, Copy, Merge, ScanSearch, ShieldCheck, Trash2 } from "lucide-react";
import { useAppStore } from "../app/store";
import { api } from "../lib/tauri/api";
import { formatBytes } from "../lib/format";
import type { DuplicateGroup, ModelSummary } from "../types";
import { ModelThumbnail } from "./ModelThumbnail";
import { Dialog } from "./Dialog";
import { plural, t } from "../lib/i18n";

export function DuplicateStacks({ groups, loading, loadingMore, hasMore, onLoadMore }: { groups: DuplicateGroup[]; loading: boolean; loadingMore: boolean; hasMore: boolean; onLoadMore: () => void }) {
  const selectModel = useAppStore((state) => state.selectModel);
  const [reviewing, setReviewing] = useState<DuplicateGroup>();
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
  if (!groups.length) return <div className="empty-state"><span className="empty-state__icon"><Box size={24} /></span><h2>{t("No duplicate projects")}</h2><p>{t("Volum compares file contents and geometry, not only filenames.")}</p></div>;
  return <><div className="duplicate-grid-scroll" ref={scrollRef}><div className="duplicate-grid">{groups.map((group) => { const representative = group.models[0]; const folders = [...new Set(group.models.map((model) => model.relativeFolder || model.folderName))]; return <article className="duplicate-stack" key={group.id}><button className="duplicate-stack__preview" onClick={() => representative && selectModel(representative.id)}>{representative && <ModelThumbnail modelId={representative.id} assetId={representative.primaryAssetId} extension={representative.primaryExtension} revision={representative.modifiedAt} missing={representative.missing} />}<span className="duplicate-stack__layer duplicate-stack__layer--one" /><span className="duplicate-stack__layer duplicate-stack__layer--two" /><span className="duplicate-stack__count">{group.matchKind === "geometry" ? <ScanSearch size={12} /> : <Copy size={12} />}{group.modelCount}</span></button><div className="duplicate-stack__copy"><span className={`duplicate-kind is-${group.matchKind}`}>{t(group.matchKind === "exact" ? "Exact files" : "Same geometry")}</span><h3>{representative?.displayName ?? t("Duplicate projects")}</h3><p>{plural(group.modelCount, "{count} matching project", "{count} matching projects")} · {formatBytes(group.byteSize)}</p><small>{folders.slice(0, 2).join(" · ")}{folders.length > 2 ? ` +${folders.length - 2}` : ""}</small></div><button className="button button--quiet button--full" onClick={() => setReviewing(group)}><ShieldCheck size={14} /> {t("Review and clean up")}</button></article>; })}</div>{hasMore && <div className="duplicate-grid__sentinel" ref={sentinelRef}>{t(loadingMore ? "Loading more stacks…" : "Scroll to load more")}</div>}</div>{reviewing && <CleanupDialog group={reviewing} onClose={() => setReviewing(undefined)} />}</>;
}

function keeperScore(model: ModelSummary) {
  const format = model.primaryExtension === "step" || model.primaryExtension === "stp" ? 4 : model.primaryExtension === "3mf" ? 3 : model.primaryExtension === "obj" ? 2 : 1;
  return format * 100 + model.assetCount * 10 + (model.favorite ? 5 : 0) + (model.missing ? -1000 : 0);
}

function CleanupDialog({ group, onClose }: { group: DuplicateGroup; onClose: () => void }) {
  const queryClient = useQueryClient();
  const recommended = useMemo(() => [...group.models].sort((a, b) => keeperScore(b) - keeperScore(a))[0], [group.models]);
  const [keeperId, setKeeperId] = useState(recommended?.id ?? group.models[0]?.id ?? "");
  const [moveToTrash, setMoveToTrash] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState("");
  const clean = async () => {
    if (moveToTrash && !window.confirm(t("Move the redundant matching files to Trash? You can restore them from the operating system’s Trash."))) return;
    setSaving(true);
    setError("");
    try {
      await api.cleanupDuplicate({ matchKey: group.matchKey, keeperId, duplicateIds: group.models.filter((model) => model.id !== keeperId).map((model) => model.id), moveToTrash });
      await Promise.all([queryClient.invalidateQueries({ queryKey: ["duplicate-groups"] }), queryClient.invalidateQueries({ queryKey: ["duplicate-stats"] }), queryClient.invalidateQueries({ queryKey: ["models"] }), queryClient.invalidateQueries({ queryKey: ["collections"] })]);
      onClose();
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setSaving(false);
    }
  };
  const dismiss = async () => {
    setError("");
    try {
      await api.dismissDuplicate(group.matchKey);
      await Promise.all([queryClient.invalidateQueries({ queryKey: ["duplicate-groups"] }), queryClient.invalidateQueries({ queryKey: ["duplicate-stats"] })]);
      onClose();
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    }
  };
  return <Dialog title={t("Choose the project to keep")} subtitle={t(group.matchKind === "exact" ? "These projects contain byte-for-byte identical files." : "These files encode the same measured geometry despite differing file data.")} onClose={onClose} size="medium"><div className="cleanup-guide"><div className="cleanup-confidence"><ShieldCheck size={17} /><span><strong>{t(group.matchKind === "exact" ? "Exact match" : "Geometry match")}</strong><small>{t("Confidence: {percent}%", { percent: Math.round(group.confidence * 100) })}</small></span></div><div className="keeper-list">{group.models.map((model) => <label className={keeperId === model.id ? "is-selected" : ""} key={model.id}><input type="radio" name="keeper" checked={keeperId === model.id} onChange={() => setKeeperId(model.id)} /><span className="file-icon">{model.primaryExtension.toUpperCase()}</span><span><strong>{model.displayName}</strong><small>{model.relativeFolder || model.folderName} · {plural(model.assetCount, "{count} file", "{count} files")}</small></span>{model.id === recommended?.id && <em><Check size={11} /> {t("Recommended")}</em>}</label>)}</div><p className="quiet-copy">{t("Tags, collections, notes, favorites, estimates, web attribution, and files will be transferred to the keeper project.")}</p><label className="cleanup-trash"><input type="checkbox" checked={moveToTrash} onChange={(event) => setMoveToTrash(event.target.checked)} /><Trash2 size={15} /><span><strong>{t("Move redundant files to Trash")}</strong><small>{t("Leave this off to bundle every matching file into the keeper project without deleting anything.")}</small></span></label>{error && <div className="form-error">{error}</div>}<div className="dialog__actions"><button className="button button--quiet" disabled={saving} onClick={() => void dismiss()}>{t("Not duplicates")}</button><button className="button button--primary" disabled={!keeperId || saving} onClick={() => void clean()}><Merge size={15} /> {t(saving ? "Cleaning up…" : "Consolidate projects")}</button></div></div></Dialog>;
}
