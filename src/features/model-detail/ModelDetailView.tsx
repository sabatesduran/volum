import { useEffect, useMemo, useRef, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  ArrowLeft, Box, Check, ChevronDown, Clock3, Copy, ExternalLink, FileBox, FolderOpen, Globe2,
  GitBranch, Heart, Layers3, LoaderCircle, Plus, Printer, RotateCcw, Save, Scale, ScanSearch, Scissors, Star, Tags, TriangleAlert, X
} from "lucide-react";
import { useAppStore } from "../../app/store";
import { ModelViewer, type ModelView, type ViewerGeometry } from "../../components/ModelViewer";
import { api } from "../../lib/tauri/api";
import { formatBytes, formatCurrency, formatDate, formatDimensions, formatDuration, formatNumber, titleCase } from "../../lib/format";
import { enabledSlicers, parseSlicerConfig } from "../../lib/slicers";
import { SlicerIcon } from "../../components/SlicerIcon";
import type { CostEstimate, Material, SlicerApplication, ThreeMfMetadata, ThreeMfPlate } from "../../types";
import { intlLocale, plural, t } from "../../lib/i18n";

function OpenInSlicer({ assetId, disabled, applications, defaultId }: { assetId?: string; disabled: boolean; applications: SlicerApplication[]; defaultId?: string }) {
  const [menuOpen, setMenuOpen] = useState(false);
  const root = useRef<HTMLDivElement>(null);
  const primary = applications.find((application) => application.id === defaultId) ?? applications[0];
  useEffect(() => {
    if (!menuOpen) return;
    const close = (event: PointerEvent) => { if (!root.current?.contains(event.target as Node)) setMenuOpen(false); };
    const closeOnEscape = (event: KeyboardEvent) => { if (event.key === "Escape") setMenuOpen(false); };
    document.addEventListener("pointerdown", close);
    document.addEventListener("keydown", closeOnEscape);
    return () => { document.removeEventListener("pointerdown", close); document.removeEventListener("keydown", closeOnEscape); };
  }, [menuOpen]);
  const launch = (application?: SlicerApplication) => {
    if (assetId) void api.openAsset(assetId, application?.path);
    setMenuOpen(false);
  };
  return <div className="open-slicer" ref={root}><button className="open-slicer__main" disabled={disabled} onClick={() => launch(primary)}>{primary ? <SlicerIcon application={primary} size={23} /> : <ExternalLink size={17} />}<span>{primary ? t("Open in {name}", { name: primary.name }) : t("Open in default app")}</span></button><button className="open-slicer__toggle" disabled={disabled} onClick={() => setMenuOpen((open) => !open)} aria-label={t("Choose slicer")} aria-expanded={menuOpen}><ChevronDown size={16} /></button>{menuOpen && <div className="open-slicer__menu"><div className="open-slicer__menu-label">{t("Open with")}</div>{applications.map((application) => <button key={application.id} onClick={() => launch(application)}><SlicerIcon application={application} size={25} /><span><strong>{application.name}</strong><small>{t(application.id === defaultId ? "Default slicer" : "Configured slicer")}</small></span>{application.id === defaultId && <Check size={14} />}</button>)}{applications.length > 0 && <div className="open-slicer__separator" />}<button onClick={() => launch()}><span className="open-slicer__system"><ExternalLink size={14} /></span><span><strong>{t("System default")}</strong><small>{t("Use the operating system association")}</small></span></button></div>}</div>;
}

function DetailSection({ title, icon, children, open = true }: { title: string; icon?: React.ReactNode; children: React.ReactNode; open?: boolean }) {
  return <details className="detail-section" open={open}><summary>{icon}<span>{title}</span><ChevronDown size={16} /></summary><div className="detail-section__body">{children}</div></details>;
}

function CostCalculator({ modelId, assetId, suggestedGrams, suggestedSource = "file", estimate }: { modelId: string; assetId?: string; suggestedGrams?: number; suggestedSource?: "file" | "web"; estimate?: CostEstimate }) {
  const queryClient = useQueryClient();
  const { data: materials = [] } = useQuery({ queryKey: ["materials"], queryFn: api.materials });
  const defaultMaterial = materials[0];
  const savedEstimate = estimate && (!estimate.assetId || estimate.assetId === assetId) ? estimate : undefined;
  const parsedGrams = suggestedGrams != null && Number.isFinite(suggestedGrams) && suggestedGrams > 0 ? suggestedGrams : undefined;
  const [grams, setGrams] = useState("");
  const [usageSource, setUsageSource] = useState<"file" | "web" | "manual">("manual");
  const [quantity, setQuantity] = useState(1);
  const [materialId, setMaterialId] = useState("");
  const [price, setPrice] = useState(15.99);
  const [weight, setWeight] = useState(1000);
  const [currency, setCurrency] = useState("EUR");
  useEffect(() => {
    const initialGrams = savedEstimate?.plasticG ?? parsedGrams;
    setGrams(initialGrams == null ? "" : String(Math.round(initialGrams * 100) / 100));
    setUsageSource(savedEstimate?.source === "file" || savedEstimate?.source === "web" ? savedEstimate.source : !savedEstimate && parsedGrams != null ? suggestedSource : "manual");
    setQuantity(savedEstimate?.quantity ?? 1);
    if (savedEstimate?.materialId) setMaterialId(savedEstimate.materialId);
  }, [assetId, parsedGrams, savedEstimate?.id, savedEstimate?.materialId, savedEstimate?.plasticG, savedEstimate?.quantity, savedEstimate?.source, suggestedSource]);
  useEffect(() => {
    const material = materials.find((item) => item.id === materialId) ?? defaultMaterial;
    if (material) {
      setMaterialId(material.id);
      setPrice(material.spoolPriceMinor / 100);
      setWeight(material.spoolWeightG);
      setCurrency(material.currency);
    }
  }, [defaultMaterial, materialId, materials]);
  const calculated = useMemo(() => {
    const plasticG = Number(grams);
    if (!Number.isFinite(plasticG) || plasticG <= 0) return undefined;
    const costPerPieceMinor = plasticG / Math.max(weight, 1) * price * 100;
    return { plasticG, costPerPieceMinor, batchCostMinor: costPerPieceMinor * quantity };
  }, [grams, quantity, price, weight]);
  const save = async () => {
    if (!calculated) return;
    await api.saveEstimate({ modelId, assetId, materialId: materialId || undefined, plasticG: calculated.plasticG, quantity, source: usageSource });
    queryClient.invalidateQueries({ queryKey: ["model", modelId] });
  };
  return (
    <div className="cost-calculator">
      <div className="calculator-grid">
        <label className="field field--compact"><span>{t("Plastic used")}</span><div className="input-unit"><input type="number" min="0" step=".01" value={grams} placeholder={t("Enter weight")} onChange={(event) => { setGrams(event.target.value); setUsageSource("manual"); }} /><span>g</span></div><small>{t(usageSource === "file" ? "From sliced 3MF" : usageSource === "web" ? "From the default web print profile" : grams ? "Manual value" : "Not available in this file")}</small></label>
        <label className="field field--compact"><span>{t("Quantity")}</span><input type="number" min="1" value={quantity} onChange={(event) => setQuantity(Math.max(1, Number(event.target.value)))} /></label>
      </div>
      {materials.length > 0 && <label className="field field--compact"><span>{t("Material")}</span><select value={materialId || defaultMaterial?.id} onChange={(event) => setMaterialId(event.target.value)}>{materials.map((material) => <option key={material.id} value={material.id}>{material.name} · {formatCurrency(material.spoolPriceMinor, material.currency)}</option>)}</select></label>}
      <div className="calculator-grid">
        <label className="field field--compact"><span>{t("Spool price")}</span><div className="input-unit input-unit--prefix"><span>{currency === "EUR" ? "€" : currency === "USD" ? "$" : currency}</span><input type="number" min="0" step=".01" value={price} onChange={(event) => setPrice(Number(event.target.value))} /></div></label>
        <label className="field field--compact"><span>{t("Spool weight")}</span><div className="input-unit"><input type="number" min="1" value={weight} onChange={(event) => setWeight(Number(event.target.value))} /><span>g</span></div></label>
      </div>
      <div className="cost-result"><div><span>{t("One piece")}</span><strong>{calculated ? formatCurrency(calculated.costPerPieceMinor, currency) : "—"}</strong></div><div className="cost-result__batch"><span>{t("Batch · {count}", { count: quantity })}</span><strong>{calculated ? formatCurrency(calculated.batchCostMinor, currency) : "—"}</strong></div></div>
      <button className="button button--quiet button--full" disabled={!calculated} onClick={save}><Save size={15} /> {t("Save estimate")}</button>
    </div>
  );
}

function CollectionPicker({ modelId, selected }: { modelId: string; selected: string[] }) {
  const queryClient = useQueryClient();
  const { data: collections = [] } = useQuery({ queryKey: ["collections"], queryFn: api.collections });
  const toggle = async (collectionId: string) => {
    if (selected.includes(collectionId)) await api.removeFromCollection(collectionId, [modelId]);
    else await api.addToCollection(collectionId, [modelId]);
    await Promise.all([
      queryClient.invalidateQueries({ queryKey: ["model", modelId] }),
      queryClient.invalidateQueries({ queryKey: ["collections"] })
    ]);
  };
  const manual = collections.filter((collection) => !collection.smart);
  if (!manual.length) return <p className="quiet-copy">{t("Create a manual collection to organize this project without moving its files.")}</p>;
  return <div className="collection-checklist">{manual.map((collection) => <button key={collection.id} onClick={() => toggle(collection.id)}><span className="collection-color" style={{ background: collection.color }} />{collection.name}<span className={`check-box ${selected.includes(collection.id) ? "is-checked" : ""}`}>{selected.includes(collection.id) && <Check size={12} />}</span></button>)}</div>;
}

const tagColors = ["#ff5a36", "#2f8f64", "#8f6ac8", "#d59b2f", "#397aa8", "#b75276"];

function TagEditor({ modelId, selected }: { modelId: string; selected: Array<{ id: string; name: string; color: string }> }) {
  const queryClient = useQueryClient();
  const { data: tags = [] } = useQuery({ queryKey: ["tags"], queryFn: api.tags });
  const [name, setName] = useState("");
  const update = async (ids: string[]) => {
    await api.setModelTags(modelId, ids);
    await Promise.all([
      queryClient.invalidateQueries({ queryKey: ["model", modelId] }),
      queryClient.invalidateQueries({ queryKey: ["tags"] }),
      queryClient.invalidateQueries({ queryKey: ["models"] }),
      queryClient.invalidateQueries({ queryKey: ["collections"] })
    ]);
  };
  const toggle = (id: string) => void update(selected.some((tag) => tag.id === id) ? selected.filter((tag) => tag.id !== id).map((tag) => tag.id) : [...selected.map((tag) => tag.id), id]);
  const create = async (event: React.FormEvent) => {
    event.preventDefault();
    const trimmed = name.trim();
    if (!trimmed) return;
    const existing = tags.find((tag) => tag.name.localeCompare(trimmed, undefined, { sensitivity: "accent" }) === 0);
    const tag = existing ?? await api.saveTag({ name: trimmed, color: tagColors[tags.length % tagColors.length] });
    if (!selected.some((item) => item.id === tag.id)) await update([...selected.map((item) => item.id), tag.id]);
    setName("");
  };
  return <div className="tag-editor"><div className="tag-chips">{selected.map((tag) => <button key={tag.id} style={{ "--tag-color": tag.color } as React.CSSProperties} onClick={() => toggle(tag.id)}>{tag.name}<X size={11} /></button>)}{!selected.length && <span className="quiet-copy">{t("No tags yet.")}</span>}</div><form onSubmit={create} className="tag-add"><input value={name} onChange={(event) => setName(event.target.value)} list={`tags-${modelId}`} placeholder={t("Add or create a tag…")} maxLength={40} /><datalist id={`tags-${modelId}`}>{tags.filter((tag) => !selected.some((item) => item.id === tag.id)).map((tag) => <option key={tag.id} value={tag.name} />)}</datalist><button type="submit" className="icon-button" disabled={!name.trim()} aria-label={t("Add tag")}><Plus size={15} /></button></form>{tags.some((tag) => !selected.some((item) => item.id === tag.id)) && <div className="tag-suggestions">{tags.filter((tag) => !selected.some((item) => item.id === tag.id)).slice(0, 8).map((tag) => <button key={tag.id} onClick={() => toggle(tag.id)}><span style={{ background: tag.color }} />{tag.name}</button>)}</div>}</div>;
}

function PlateThumbnail({ assetId, plate }: { assetId: string; plate: ThreeMfPlate }) {
  const { data } = useQuery({ queryKey: ["plate-thumbnail", assetId, plate.index], queryFn: () => api.plateThumbnail(assetId, plate.index), enabled: plate.thumbnail });
  const [source, setSource] = useState<string>();
  useEffect(() => {
    if (!data?.length) { setSource(undefined); return; }
    const url = URL.createObjectURL(new Blob([new Uint8Array(data)], { type: "image/png" }));
    setSource(url);
    return () => URL.revokeObjectURL(url);
  }, [data]);
  return <div className="plate-thumbnail">{source ? <img src={source} alt={t("Plate {index}", { index: plate.index })} /> : <span><Box size={18} />{t("Plate {index}", { index: plate.index })}</span>}</div>;
}

function ThreeMfInspector({ assetId, metadata, selectedPlateIndex, onSelectPlate, onShowAll }: { assetId: string; metadata: ThreeMfMetadata; selectedPlateIndex?: number; onSelectPlate: (index: number) => void; onShowAll: () => void }) {
  const plates = metadata.plates ?? [];
  return <div className="three-mf-inspector"><dl className="metadata-list metadata-list--profile">{metadata.printer && <div><dt>{t("Printer")}</dt><dd>{metadata.printer}</dd></div>}{metadata.printProfile && <div><dt>{t("Profile")}</dt><dd>{metadata.printProfile}</dd></div>}{metadata.nozzleDiameterMm != null && <div><dt>{t("Nozzle")}</dt><dd>{formatNumber(metadata.nozzleDiameterMm)} mm</dd></div>}{metadata.layerHeightMm != null && <div><dt>{t("Layer height")}</dt><dd>{formatNumber(metadata.layerHeightMm)} mm</dd></div>}{metadata.slicer && <div><dt>{t("Created with")}</dt><dd>{metadata.slicer}{metadata.slicerVersion ? ` · ${metadata.slicerVersion}` : ""}</dd></div>}</dl>{plates.length > 0 ? <div className="plate-list"><button type="button" className={`plate-card plate-card--all ${selectedPlateIndex == null ? "is-active" : ""}`} onClick={onShowAll}><span className="plate-thumbnail plate-thumbnail--all"><Box size={20} /></span><span className="plate-card__copy"><span className="plate-card__title"><strong>{t("All plates")}</strong><span>{plural(plates.length, "{count} plate", "{count} plates")}</span></span><p>{t("Show the complete 3MF project")}</p></span></button>{plates.map((plate) => { const objectNames = plate.objectNames ?? []; return <button type="button" className={`plate-card ${selectedPlateIndex === plate.index ? "is-active" : ""}`} key={plate.index} onClick={() => onSelectPlate(plate.index)}><PlateThumbnail assetId={assetId} plate={plate} /><div className="plate-card__copy"><div className="plate-card__title"><strong>{plate.name || t("Plate {index}", { index: plate.index })}</strong><span>{plural(plate.objectCount, "{count} object", "{count} objects")}</span></div>{objectNames.length > 0 && <p>{objectNames.slice(0, 3).join(" · ")}{objectNames.length > 3 ? ` +${objectNames.length - 3}` : ""}</p>}<div className="plate-facts">{plate.printTimeSeconds != null && <span><Clock3 size={11} />{formatDuration(plate.printTimeSeconds)}</span>}{plate.filamentGrams != null && <span><Scale size={11} />{formatNumber(plate.filamentGrams, { minimumFractionDigits: 1, maximumFractionDigits: 1 })} g</span>}{(plate.materialNames ?? []).map((material) => <span key={material}>{material}</span>)}{plate.bedType && <span>{plate.bedType}</span>}</div></div></button>; })}</div> : <p className="quiet-copy">{t("This 3MF contains a model but no plate definitions.")}</p>}</div>;
}

export function ModelDetailView({ modelId }: { modelId: string }) {
  const queryClient = useQueryClient();
  const selectModel = useAppStore((state) => state.selectModel);
  const { data: model, isLoading, error } = useQuery({ queryKey: ["model", modelId], queryFn: () => api.model(modelId) });
  const { data: related = [] } = useQuery({ queryKey: ["related-models", modelId], queryFn: () => api.relatedModels(modelId) });
  const { data: preferences = {} } = useQuery({ queryKey: ["preferences"], queryFn: api.preferences });
  const slicerConfig = useMemo(() => parseSlicerConfig(preferences), [preferences]);
  const customSlicerKey = slicerConfig.customApps.map((app) => `${app.id}:${app.path}`).join("|");
  const { data: slicerApplications = [] } = useQuery({ queryKey: ["slicers", customSlicerKey], queryFn: () => api.slicers(slicerConfig.customApps) });
  const availableSlicers = useMemo(() => enabledSlicers(slicerConfig, slicerApplications), [slicerApplications, slicerConfig]);
  const [activeAssetId, setActiveAssetId] = useState<string>();
  const [resetSignal, setResetSignal] = useState(0);
  const [cameraView, setCameraView] = useState<ModelView>("iso");
  const [activePlateIndex, setActivePlateIndex] = useState<number>();
  const [viewerGeometry, setViewerGeometry] = useState<ViewerGeometry>();
  const [previewError, setPreviewError] = useState("");
  const [notes, setNotes] = useState("");
  const [splitAssetIds, setSplitAssetIds] = useState<string[]>([]);
  useEffect(() => {
    if (model) {
      setActiveAssetId(model.primaryAssetId ?? model.assets[0]?.id);
      setNotes(model.notes);
      setSplitAssetIds([]);
    }
  }, [model]);
  const activeAsset = model?.assets.find((asset) => asset.id === activeAssetId) ?? model?.assets[0];
  const viewerPlates = activeAsset?.metadata.threeMf?.plates ?? [];
  useEffect(() => {
    setViewerGeometry(undefined);
    setActivePlateIndex(viewerPlates.length > 1 ? (viewerPlates.find((plate) => plate.objectCount > 0)?.index ?? viewerPlates[0]?.index) : undefined);
  }, [activeAsset?.id]);
  const fileFilamentGrams = activeAsset?.metadata.filamentGrams;
  const suggestedFilamentGrams = fileFilamentGrams ?? model?.webSource?.filamentGrams;
  const suggestedFilamentSource = fileFilamentGrams != null ? "file" : "web";
  const favorite = useMutation({ mutationFn: () => api.toggleFavorite(modelId), onSuccess: () => { queryClient.invalidateQueries({ queryKey: ["model", modelId] }); queryClient.invalidateQueries({ queryKey: ["models"] }); } });
  const refreshProject = async () => {
    await Promise.all([queryClient.invalidateQueries({ queryKey: ["model"] }), queryClient.invalidateQueries({ queryKey: ["models"] }), queryClient.invalidateQueries({ queryKey: ["related-models"] }), queryClient.invalidateQueries({ queryKey: ["duplicate-groups"] })]);
  };
  const splitFiles = async () => {
    const name = window.prompt(t("Name the new project"));
    if (!name?.trim()) return;
    const newId = await api.splitProject(modelId, splitAssetIds, name.trim());
    await refreshProject();
    selectModel(newId);
  };

  if (isLoading) return <div className="detail-loading"><LoaderCircle className="spin" /><span>{t("Preparing project…")}</span></div>;
  if (error || !model) return <div className="empty-state"><TriangleAlert /><h2>{t("Couldn’t open this project")}</h2><p>{error instanceof Error ? error.message : t("The project may have moved.")}</p><button className="button" onClick={() => selectModel()}>{t("Back to library")}</button></div>;

  return (
    <div className="model-detail">
      <section className="model-detail__stage">
        <div className="detail-toolbar">
          <button className="button button--floating" onClick={() => selectModel()}><ArrowLeft size={17} /> {t("Back")}</button>
          <div className="detail-toolbar__right">
            <button className={`button button--floating button--icon ${model.favorite ? "is-favorite" : ""}`} onClick={() => favorite.mutate()} aria-label={t("Favorite")}><Heart size={17} fill={model.favorite ? "currentColor" : "none"} /></button>
            <button className="button button--floating" onClick={() => { setCameraView("iso"); setResetSignal((value) => value + 1); }}><RotateCcw size={16} /> {t("Reset view")}</button>
          </div>
        </div>
        <div className="viewer-wrap">
          <ModelViewer assetId={activeAsset?.id} extension={activeAsset?.extension ?? model.primaryExtension} modelId={model.id} plateIndex={activePlateIndex} resetSignal={resetSignal} view={cameraView} onError={setPreviewError} onGeometry={setViewerGeometry} />
          {previewError && <div className="viewer-error"><TriangleAlert size={15} />{previewError}</div>}
          {viewerPlates.length > 1 && <div className="viewer-plate-controls" role="group" aria-label={t("3MF plate")}><button type="button" className={activePlateIndex == null ? "is-active" : ""} onClick={() => setActivePlateIndex(undefined)}>{t("All plates")}</button>{viewerPlates.map((plate) => <button type="button" key={plate.index} className={activePlateIndex === plate.index ? "is-active" : ""} onClick={() => setActivePlateIndex(plate.index)}>{plate.name || t("Plate {index}", { index: plate.index })}</button>)}</div>}
          <div className="viewer-view-controls" role="group" aria-label={t("Model view")}>{(["iso", "front", "side", "top"] as ModelView[]).map((view) => <button type="button" key={view} className={cameraView === view ? "is-active" : ""} onClick={() => setCameraView(view)}>{t(view === "iso" ? "Isometric" : view === "front" ? "Front" : view === "side" ? "Side" : "Top")}</button>)}</div>
          <div className="viewer-hint">{t("Drag to rotate · Scroll to zoom")}</div>
        </div>
        {model.assets.length > 1 && <div className="variant-strip">{model.assets.map((asset) => <button className={asset.id === activeAsset?.id ? "is-active" : ""} key={asset.id} onClick={() => { setActiveAssetId(asset.id); setPreviewError(""); }}><FileBox size={16} /><span>{asset.extension.toUpperCase()}</span></button>)}</div>}
      </section>
      <aside className="model-detail__inspector">
        <header className="inspector-header">
          <div className="eyebrow">{t("{format} project · {count} files", { format: model.primaryExtension.toUpperCase(), count: model.assetCount })}</div>
          <h1>{model.displayName}</h1>
          <button className="breadcrumb-button" onClick={() => { useAppStore.getState().selectFolder(model.folderId); }}><FolderOpen size={14} />{model.rootName} / {model.relativeFolder}</button>
        </header>
        <div className="primary-actions">
          <OpenInSlicer assetId={activeAsset?.id} disabled={!activeAsset || activeAsset.missing} applications={availableSlicers} defaultId={slicerConfig.defaultId} />
          <button className="button button--quiet" disabled={!activeAsset || activeAsset.missing} onClick={() => activeAsset && api.revealAsset(activeAsset.id)}><FolderOpen size={16} /> {t("Reveal")}</button>
        </div>
        <div className="inspector-scroll">
          <DetailSection title={t("Details")} icon={<Scale size={16} />}>
            <dl className="metadata-list"><div><dt>{t("Dimensions")}</dt><dd>{formatDimensions(activeAsset?.metadata.dimensionsMm ?? viewerGeometry?.dimensionsMm ?? model.dimensionsMm)}</dd></div><div><dt>{t("File size")}</dt><dd>{activeAsset ? formatBytes(activeAsset.byteSize) : "—"}</dd></div><div><dt>{t("Modified")}</dt><dd>{formatDate(activeAsset?.modifiedAt)}</dd></div><div><dt>{t("Format")}</dt><dd>{activeAsset?.extension.toUpperCase()}</dd></div>{activeAsset?.metadata.surfaceAreaMm2 != null && <div><dt>{t("Surface area")}</dt><dd>{formatNumber(activeAsset.metadata.surfaceAreaMm2, { maximumFractionDigits: 1 })} mm²</dd></div>}{activeAsset?.metadata.volumeMm3 != null && <div><dt>{t("Volume")}</dt><dd>{formatNumber(activeAsset.metadata.volumeMm3, { maximumFractionDigits: 1 })} mm³</dd></div>}{(activeAsset?.metadata.triangleCount ?? viewerGeometry?.triangleCount ?? 0) > 0 && <div><dt>{t("Triangles")}</dt><dd>{(activeAsset?.metadata.triangleCount || viewerGeometry?.triangleCount || 0).toLocaleString(intlLocale)}</dd></div>}</dl>
          </DetailSection>
          {model.webSource && <DetailSection title={t("Source · {provider}", { provider: model.webSource.provider === "makerworld" ? "MakerWorld" : "Printables" })} icon={<Globe2 size={16} />}><div className="model-web-source">{model.webSource.imageUrl && <img src={model.webSource.imageUrl} alt="" referrerPolicy="no-referrer" />}<div><strong>{model.webSource.title}</strong>{model.webSource.creator && <small>{t("by {name}", { name: model.webSource.creator })}</small>}{model.webSource.license && <span>{model.webSource.license}</span>}{model.webSource.filamentGrams != null && <span>{formatNumber(model.webSource.filamentGrams, { minimumFractionDigits: 1, maximumFractionDigits: 1 })} g · {t("default print profile")}</span>}</div><button className="button button--quiet" onClick={() => api.openExternal(model.webSource!.canonicalUrl)}><ExternalLink size={14} /> {t("Open source")}</button></div></DetailSection>}
          {activeAsset?.metadata.threeMf && <DetailSection title={t("3MF plates & profile · {count}", { count: activeAsset.metadata.threeMf.plates?.length ?? 0 })} icon={<Printer size={16} />}><ThreeMfInspector assetId={activeAsset.id} metadata={activeAsset.metadata.threeMf} selectedPlateIndex={activePlateIndex} onSelectPlate={setActivePlateIndex} onShowAll={() => setActivePlateIndex(undefined)} /></DetailSection>}
          <DetailSection title={t("Project files · {count}", { count: model.assets.length })} icon={<Layers3 size={16} />}>
            <p className="quiet-copy">{t("Keep printable files, source geometry, plates, and versions together in one project.")}</p>
            <div className="asset-list asset-list--project">{model.assets.map((asset) => <div key={asset.id} className={asset.id === activeAsset?.id ? "is-active" : ""}><label className="asset-select"><input type="checkbox" checked={splitAssetIds.includes(asset.id)} onChange={(event) => setSplitAssetIds((current) => event.target.checked ? [...current, asset.id] : current.filter((id) => id !== asset.id))} aria-label={t("Select {filename}", { filename: asset.filename })} /></label><button className="asset-list__main" onClick={() => setActiveAssetId(asset.id)}><span className="file-icon">{asset.extension.toUpperCase()}</span><span><strong>{asset.filename}</strong><small>{formatBytes(asset.byteSize)} · {t(titleCase(asset.role))} · {t(titleCase(asset.parseStatus))}</small></span></button><button className={`icon-button icon-button--tiny ${asset.id === model.primaryAssetId ? "is-favorite" : ""}`} disabled={asset.id === model.primaryAssetId || asset.missing} title={t(asset.id === model.primaryAssetId ? "Primary preview" : asset.missing ? "File unavailable" : "Use as primary preview")} onClick={async () => { await api.setProjectPrimary(model.id, asset.id); await refreshProject(); }}><Star size={13} fill={asset.id === model.primaryAssetId ? "currentColor" : "none"} /></button></div>)}</div>
            {splitAssetIds.length > 0 && splitAssetIds.length < model.assets.length && <button className="button button--quiet button--full" onClick={() => void splitFiles()}><Scissors size={14} /> {t("Split selected files into a new project")}</button>}
          </DetailSection>
          <DetailSection title={`${t("Tags")}${model.tags.length ? ` · ${model.tags.length}` : ""}`} icon={<Tags size={16} />}><TagEditor modelId={model.id} selected={model.tags} /></DetailSection>
          <DetailSection title={t("Collections")} icon={<Box size={16} />}><CollectionPicker modelId={model.id} selected={model.collectionIds} /></DetailSection>
          {related.length > 0 && <DetailSection title={t("Copies & versions · {count}", { count: related.length })} icon={<GitBranch size={16} />}><div className="related-models">{related.map((item) => <div className="related-models__row" key={item.id}><button onClick={() => selectModel(item.id)}><span className={`related-models__icon is-${item.relationship}`}>{item.relationship === "duplicate" ? <Copy size={14} /> : item.relationship === "geometry" ? <ScanSearch size={14} /> : <GitBranch size={14} />}</span><span><strong>{item.displayName}</strong><small>{t(item.relationship === "duplicate" ? "Exact duplicate" : item.relationship === "geometry" ? "Same geometry" : "Possible version")} · {item.relativeFolder || t("Library root")}</small></span><span className="file-pill">{item.primaryExtension.toUpperCase()}</span></button><button className="icon-button icon-button--tiny" title={t("Add to this project")} aria-label={t("Add {name} to this project", { name: item.displayName })} onClick={async () => { if (!window.confirm(t("Bundle this project and transfer its organization metadata?"))) return; await api.mergeProjects(model.id, [item.id]); await refreshProject(); }}><Layers3 size={14} /></button></div>)}</div></DetailSection>}
          <DetailSection title={t("Material cost")} icon={<Scale size={16} />}><CostCalculator modelId={model.id} assetId={activeAsset?.id} suggestedGrams={suggestedFilamentGrams} suggestedSource={suggestedFilamentSource} estimate={model.estimate} /></DetailSection>
          <DetailSection title={t("Notes")} icon={<FileBox size={16} />} open={Boolean(model.notes)}>
            <textarea className="notes-field" value={notes} onChange={(event) => setNotes(event.target.value)} placeholder={t("Add print settings, assembly notes, or anything useful…")} />
            <button className="button button--quiet button--full" disabled={notes === model.notes} onClick={async () => { await api.saveNotes(model.id, notes); queryClient.invalidateQueries({ queryKey: ["model", model.id] }); }}><Save size={15} /> {t("Save notes")}</button>
          </DetailSection>
        </div>
      </aside>
    </div>
  );
}
