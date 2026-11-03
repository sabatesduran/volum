import { useEffect, useMemo, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { open } from "@tauri-apps/plugin-dialog";
import {
  ArrowDown, ArrowUp, Check, ChevronDown, ClipboardList, ExternalLink, FileImage,
  ListChecks, Pencil, Plus, Save, Star, Trash2, X
} from "lucide-react";
import { api, isTauri } from "../../lib/tauri/api";
import { formatBytes } from "../../lib/format";
import { t } from "../../lib/i18n";
import type {
  AssemblyStep, Asset, CustomFieldDefinition, CustomFieldType, ProjectBomItem,
  ProjectCustomValue, ProjectPhoto
} from "../../types";
import { cachedObjectUrl } from "../../lib/mediaCache";

function Section({ title, icon, count, children, open = false }: { title: string; icon: React.ReactNode; count?: number; children: React.ReactNode; open?: boolean }) {
  return <details className="detail-section" open={open}><summary>{icon}<span>{title}{count ? ` · ${count}` : ""}</span><ChevronDown size={16} /></summary><div className="detail-section__body">{children}</div></details>;
}

function PhotoImage({ photo, full = false }: { photo: ProjectPhoto; full?: boolean }) {
  const [source, setSource] = useState("");
  const [visible, setVisible] = useState(false);
  const placeholderRef = useRef<HTMLSpanElement>(null);
  useEffect(() => {
    const element = placeholderRef.current;
    if (!element || typeof IntersectionObserver === "undefined") { setVisible(true); return; }
    const observer = new IntersectionObserver((entries) => {
      if (entries.some((entry) => entry.isIntersecting)) {
        setVisible(true);
        observer.disconnect();
      }
    }, { rootMargin: "500px" });
    observer.observe(element);
    return () => observer.disconnect();
  }, []);
  useEffect(() => {
    if (!visible) return;
    let active = true;
    void cachedObjectUrl(
      `project-photo:${photo.id}:${photo.updatedAt}:${full ? "full" : "thumb"}`,
      full ? photo.mimeType : "image/png",
      () => api.projectPhoto(photo.id, !full)
    ).then((url) => { if (active && url) setSource(url); }).catch(() => undefined);
    return () => { active = false; };
  }, [full, photo.id, photo.mimeType, photo.updatedAt, visible]);
  return source ? <img src={source} alt={photo.caption || photo.originalFilename} /> : <span ref={placeholderRef} className="project-photo__placeholder"><FileImage size={20} /></span>;
}

function ProjectPhotos({ modelId, photos }: { modelId: string; photos: ProjectPhoto[] }) {
  const queryClient = useQueryClient();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const refresh = async () => {
    await Promise.all([
      queryClient.invalidateQueries({ queryKey: ["project-documentation", modelId] }),
      queryClient.invalidateQueries({ queryKey: ["model", modelId] }),
      queryClient.invalidateQueries({ queryKey: ["models"] })
    ]);
  };
  const add = async () => {
    if (!isTauri()) return;
    const paths = await open({ multiple: true, filters: [{ name: t("Project photos"), extensions: ["jpg", "jpeg", "png", "webp"] }] });
    if (!paths) return;
    setBusy(true); setError("");
    try {
      for (const path of Array.isArray(paths) ? paths : [paths]) await api.addProjectPhoto(modelId, path);
      await refresh();
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    } finally { setBusy(false); }
  };
  const update = async (photo: ProjectPhoto, changes: Partial<Pick<ProjectPhoto, "caption" | "sortOrder" | "isCover">>) => {
    setError("");
    try {
      await api.updateProjectPhoto({ id: photo.id, caption: changes.caption ?? photo.caption, sortOrder: changes.sortOrder ?? photo.sortOrder, isCover: changes.isCover ?? photo.isCover });
      await refresh();
    } catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)); }
  };
  const move = async (index: number, direction: -1 | 1) => {
    const other = photos[index + direction];
    const current = photos[index];
    if (!current || !other) return;
    await api.updateProjectPhoto({ id: current.id, caption: current.caption, sortOrder: other.sortOrder, isCover: current.isCover });
    await api.updateProjectPhoto({ id: other.id, caption: other.caption, sortOrder: current.sortOrder, isCover: other.isCover });
    await refresh();
  };
  const remove = async (photo: ProjectPhoto) => {
    if (!window.confirm(t("Remove this managed project photo?"))) return;
    await api.deleteProjectPhoto(photo.id);
    await refresh();
  };
  return <Section title={t("Project photos")} count={photos.length} icon={<FileImage size={16} />} open={photos.length > 0}>
    <div className="section-intro"><p className="quiet-copy">{t("JPEG, PNG, and WebP photos are copied into Volum and included in backups and project packages.")}</p><button className="button button--quiet" disabled={busy || !isTauri()} onClick={() => void add()}><Plus size={14} />{t(busy ? "Adding…" : "Add photos")}</button></div>
    {error && <p className="form-error">{error}</p>}
    {photos.length ? <div className="project-photo-grid">{photos.map((photo, index) => <article className="project-photo" key={photo.id}>
      <div className="project-photo__image"><PhotoImage photo={photo} />{photo.isCover && <span className="project-photo__cover"><Star size={11} fill="currentColor" />{t("Cover")}</span>}</div>
      <input defaultValue={photo.caption} key={photo.updatedAt} maxLength={500} placeholder={t("Add a caption…")} onBlur={(event) => { if (event.target.value !== photo.caption) void update(photo, { caption: event.target.value }); }} />
      <div className="project-photo__meta"><span>{formatBytes(photo.byteSize)} · {photo.width}×{photo.height}</span><div><button className="icon-button icon-button--tiny" disabled={index === 0} onClick={() => void move(index, -1)} aria-label={t("Move photo earlier")}><ArrowUp size={13} /></button><button className="icon-button icon-button--tiny" disabled={index === photos.length - 1} onClick={() => void move(index, 1)} aria-label={t("Move photo later")}><ArrowDown size={13} /></button><button className={`icon-button icon-button--tiny ${photo.isCover ? "is-favorite" : ""}`} onClick={() => void update(photo, { isCover: true })} aria-label={t("Use as library cover")}><Star size={13} fill={photo.isCover ? "currentColor" : "none"} /></button><button className="icon-button icon-button--tiny" onClick={() => void remove(photo)} aria-label={t("Remove photo")}><Trash2 size={13} /></button></div></div>
    </article>)}</div> : <p className="quiet-copy">{t("Add finished prints, process photos, or assembly references.")}</p>}
  </Section>;
}

function emptyValue(field: CustomFieldDefinition): string | number | boolean | undefined {
  if (field.fieldType === "boolean") return undefined;
  if (field.fieldType === "number") return undefined;
  return "";
}

function CustomValueInput({ field, stored, onSave }: { field: CustomFieldDefinition; stored?: ProjectCustomValue; onSave: (value?: string | number | boolean) => void }) {
  const initial = stored?.value ?? emptyValue(field);
  const [value, setValue] = useState<string | number | boolean | undefined>(initial);
  useEffect(() => setValue(initial), [field.id, initial]);
  if (field.fieldType === "boolean") return <select value={value == null ? "" : String(value)} onChange={(event) => { const next = event.target.value === "" ? undefined : event.target.value === "true"; setValue(next); onSave(next); }}><option value="">{t("Not set")}</option><option value="true">{t("Yes")}</option><option value="false">{t("No")}</option></select>;
  if (field.fieldType === "select") return <select value={String(value ?? "")} onChange={(event) => { setValue(event.target.value); onSave(event.target.value || undefined); }}><option value="">{t("Not set")}</option>{field.options.map((option) => <option value={option} key={option}>{option}</option>)}</select>;
  const type = field.fieldType === "number" ? "number" : field.fieldType === "date" ? "date" : field.fieldType === "url" ? "url" : "text";
  return <div className="input-unit"><input type={type} value={value == null ? "" : String(value)} step={field.fieldType === "number" ? "any" : undefined} onChange={(event) => setValue(field.fieldType === "number" ? (event.target.value === "" ? undefined : Number(event.target.value)) : event.target.value)} onBlur={() => onSave(value === "" ? undefined : value)} />{field.unit && <span>{field.unit}</span>}</div>;
}

function CustomFields({ modelId, definitions, values }: { modelId: string; definitions: CustomFieldDefinition[]; values: ProjectCustomValue[] }) {
  const queryClient = useQueryClient();
  const [adding, setAdding] = useState(false);
  const [editingId, setEditingId] = useState<string>();
  const [name, setName] = useState("");
  const [fieldType, setFieldType] = useState<CustomFieldType>("text");
  const [unit, setUnit] = useState("");
  const [options, setOptions] = useState("");
  const [error, setError] = useState("");
  const valueMap = useMemo(() => new Map(values.map((value) => [value.fieldId, value])), [values]);
  const refresh = () => Promise.all([
    queryClient.invalidateQueries({ queryKey: ["project-documentation", modelId] }),
    queryClient.invalidateQueries({ queryKey: ["custom-fields"] }),
    queryClient.invalidateQueries({ queryKey: ["models"] })
  ]);
  const saveValue = async (fieldId: string, value?: string | number | boolean) => {
    try { setError(""); await api.setProjectCustomValue(modelId, fieldId, value); await refresh(); }
    catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)); }
  };
  const addField = async (event: React.FormEvent) => {
    event.preventDefault();
    try {
      setError("");
      await api.saveCustomField({ id: editingId, name: name.trim(), fieldType, unit: fieldType === "number" ? unit.trim() || undefined : undefined, options: fieldType === "select" ? options.split(",").map((item) => item.trim()).filter(Boolean) : [] });
      setName(""); setUnit(""); setOptions(""); setEditingId(undefined); setAdding(false); await refresh();
    } catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)); }
  };
  const remove = async (field: CustomFieldDefinition) => {
    if (!window.confirm(t("Delete the reusable field “{name}” and all of its project values?", { name: field.name }))) return;
    await api.deleteCustomField(field.id); await refresh();
  };
  const edit = (field: CustomFieldDefinition) => {
    setEditingId(field.id); setName(field.name); setFieldType(field.fieldType); setUnit(field.unit ?? ""); setOptions(field.options.join(", ")); setAdding(true);
  };
  const cancel = () => { setAdding(false); setEditingId(undefined); setName(""); setUnit(""); setOptions(""); };
  return <Section title={t("Custom fields")} count={definitions.length} icon={<ListChecks size={16} />} open={values.length > 0}>
    <div className="section-intro"><p className="quiet-copy">{t("Reusable typed fields can be searched and used in smart collections.")}</p><button className="button button--quiet" onClick={() => adding ? cancel() : setAdding(true)}>{adding ? <X size={14} /> : <Plus size={14} />}{t(adding ? "Cancel" : "New field")}</button></div>
    {error && <p className="form-error">{error}</p>}
    {adding && <form className="inline-editor" onSubmit={(event) => void addField(event)}><label className="field field--compact"><span>{t("Name")}</span><input autoFocus value={name} maxLength={80} onChange={(event) => setName(event.target.value)} /></label><label className="field field--compact"><span>{t("Type")}</span><select value={fieldType} onChange={(event) => setFieldType(event.target.value as CustomFieldType)}><option value="text">{t("Text")}</option><option value="number">{t("Number / unit")}</option><option value="date">{t("Date")}</option><option value="boolean">{t("Checkbox")}</option><option value="url">{t("URL")}</option><option value="select">{t("Select")}</option></select></label>{fieldType === "number" && <label className="field field--compact"><span>{t("Unit")}</span><input value={unit} maxLength={20} placeholder="mm, kg…" onChange={(event) => setUnit(event.target.value)} /></label>}{fieldType === "select" && <label className="field field--compact inline-editor__wide"><span>{t("Options")}</span><input value={options} placeholder={t("Comma-separated options")} onChange={(event) => setOptions(event.target.value)} /></label>}<button className="button button--primary" disabled={!name.trim() || (fieldType === "select" && !options.trim())}><Save size={14} />{t(editingId ? "Save field" : "Create field")}</button></form>}
    {definitions.length ? <div className="custom-field-list">{definitions.map((field) => <div className="custom-field-row" key={field.id}><span><strong>{field.name}</strong><small>{t(field.fieldType === "boolean" ? "Checkbox" : field.fieldType === "number" ? "Number / unit" : field.fieldType === "select" ? "Select" : field.fieldType.toUpperCase())}</small></span><CustomValueInput field={field} stored={valueMap.get(field.id)} onSave={(value) => void saveValue(field.id, value)} /><span className="custom-field-row__actions"><button className="icon-button icon-button--tiny" onClick={() => edit(field)} aria-label={t("Edit custom field")}><Pencil size={13} /></button><button className="icon-button icon-button--tiny" onClick={() => void remove(field)} aria-label={t("Delete custom field")}><Trash2 size={13} /></button></span></div>)}</div> : <p className="quiet-copy">{t("Create a field once, then reuse it across every project.")}</p>}
  </Section>;
}

interface BomDraft {
  id?: string;
  kind: "printed" | "purchased";
  linkedAssetId: string;
  name: string;
  quantity: string;
  unit: string;
  url: string;
  notes: string;
  sortOrder?: number;
}

const emptyBom = (): BomDraft => ({ kind: "purchased", linkedAssetId: "", name: "", quantity: "1", unit: "pcs", url: "", notes: "" });

function BillOfMaterials({ modelId, assets, items }: { modelId: string; assets: Asset[]; items: ProjectBomItem[] }) {
  const queryClient = useQueryClient();
  const [draft, setDraft] = useState<BomDraft>();
  const [error, setError] = useState("");
  const refresh = () => Promise.all([queryClient.invalidateQueries({ queryKey: ["project-documentation", modelId] }), queryClient.invalidateQueries({ queryKey: ["models"] })]);
  const edit = (item: ProjectBomItem) => setDraft({ id: item.id, kind: item.kind, linkedAssetId: item.linkedAssetId ?? "", name: item.name, quantity: String(item.quantity), unit: item.unit, url: item.url ?? "", notes: item.notes, sortOrder: item.sortOrder });
  const save = async (event: React.FormEvent) => {
    event.preventDefault(); if (!draft) return;
    try {
      setError("");
      await api.saveBomItem({ id: draft.id, modelId, kind: draft.kind, linkedAssetId: draft.linkedAssetId || undefined, name: draft.name.trim(), quantity: Number(draft.quantity), unit: draft.unit.trim(), url: draft.url.trim() || undefined, notes: draft.notes.trim(), sortOrder: draft.sortOrder });
      setDraft(undefined); await refresh();
    } catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)); }
  };
  const remove = async (item: ProjectBomItem) => { if (!window.confirm(t("Delete this BOM item?"))) return; await api.deleteBomItem(item.id); await refresh(); };
  const move = async (index: number, direction: -1 | 1) => {
    const item = items[index]; const other = items[index + direction]; if (!item || !other) return;
    const saveAt = (value: ProjectBomItem, sortOrder: number) => api.saveBomItem({ id: value.id, modelId, kind: value.kind, linkedAssetId: value.linkedAssetId, name: value.name, quantity: value.quantity, unit: value.unit, url: value.url, notes: value.notes, sortOrder });
    await saveAt(item, other.sortOrder); await saveAt(other, item.sortOrder); await refresh();
  };
  return <Section title={t("Bill of materials")} count={items.length} icon={<ClipboardList size={16} />} open={items.length > 0}>
    <div className="section-intro"><p className="quiet-copy">{t("Track printed files and purchased parts with quantities, links, and notes.")}</p><button className="button button--quiet" onClick={() => setDraft(draft ? undefined : emptyBom())}>{draft ? <X size={14} /> : <Plus size={14} />}{t(draft ? "Cancel" : "Add item")}</button></div>
    {error && <p className="form-error">{error}</p>}
    {draft && <form className="inline-editor bom-editor" onSubmit={(event) => void save(event)}><label className="field field--compact"><span>{t("Part type")}</span><select value={draft.kind} onChange={(event) => setDraft({ ...draft, kind: event.target.value as BomDraft["kind"] })}><option value="purchased">{t("Purchased")}</option><option value="printed">{t("Printed")}</option></select></label><label className="field field--compact inline-editor__wide"><span>{t("Name")}</span><input autoFocus value={draft.name} maxLength={160} onChange={(event) => setDraft({ ...draft, name: event.target.value })} /></label><label className="field field--compact"><span>{t("Quantity")}</span><input type="number" min="0.0001" step="any" value={draft.quantity} onChange={(event) => setDraft({ ...draft, quantity: event.target.value })} /></label><label className="field field--compact"><span>{t("Unit")}</span><input value={draft.unit} maxLength={24} onChange={(event) => setDraft({ ...draft, unit: event.target.value })} /></label><label className="field field--compact inline-editor__wide"><span>{t("Linked project file")}</span><select value={draft.linkedAssetId} onChange={(event) => setDraft({ ...draft, linkedAssetId: event.target.value })}><option value="">{t("No linked file")}</option>{assets.map((asset) => <option value={asset.id} key={asset.id}>{asset.filename}</option>)}</select></label><label className="field field--compact inline-editor__wide"><span>{t("Purchase link")}</span><input type="url" value={draft.url} placeholder="https://…" onChange={(event) => setDraft({ ...draft, url: event.target.value })} /></label><label className="field field--compact inline-editor__wide"><span>{t("Notes")}</span><textarea value={draft.notes} onChange={(event) => setDraft({ ...draft, notes: event.target.value })} /></label><button className="button button--primary" disabled={!draft.name.trim() || !draft.unit.trim() || Number(draft.quantity) <= 0}><Save size={14} />{t(draft.id ? "Save item" : "Add item")}</button></form>}
    {items.length ? <div className="documentation-list">{items.map((item, index) => <article className="documentation-row" key={item.id}><span className={`part-kind is-${item.kind}`}>{t(item.kind === "printed" ? "Printed" : "Purchased")}</span><div><strong>{item.quantity} {item.unit} · {item.name}</strong><small>{item.linkedAssetId ? assets.find((asset) => asset.id === item.linkedAssetId)?.filename : item.notes || t("No linked file")}</small></div>{item.url && <button className="icon-button icon-button--tiny" onClick={() => api.openExternal(item.url!)} aria-label={t("Open purchase link")}><ExternalLink size={13} /></button>}<div className="documentation-row__actions"><button className="icon-button icon-button--tiny" disabled={index === 0} onClick={() => void move(index, -1)}><ArrowUp size={13} /></button><button className="icon-button icon-button--tiny" disabled={index === items.length - 1} onClick={() => void move(index, 1)}><ArrowDown size={13} /></button><button className="icon-button icon-button--tiny" onClick={() => edit(item)}><Pencil size={13} /></button><button className="icon-button icon-button--tiny" onClick={() => void remove(item)}><Trash2 size={13} /></button></div></article>)}</div> : <p className="quiet-copy">{t("No parts have been added yet.")}</p>}
  </Section>;
}

interface StepDraft { id?: string; title: string; instructions: string; sortOrder?: number; bomItemIds: string[]; photoIds: string[] }
const emptyStep = (): StepDraft => ({ title: "", instructions: "", bomItemIds: [], photoIds: [] });

function AssemblyGuide({ modelId, steps, items, photos }: { modelId: string; steps: AssemblyStep[]; items: ProjectBomItem[]; photos: ProjectPhoto[] }) {
  const queryClient = useQueryClient();
  const [draft, setDraft] = useState<StepDraft>();
  const [error, setError] = useState("");
  const refresh = () => Promise.all([queryClient.invalidateQueries({ queryKey: ["project-documentation", modelId] }), queryClient.invalidateQueries({ queryKey: ["models"] })]);
  const save = async (event: React.FormEvent) => {
    event.preventDefault(); if (!draft) return;
    try { setError(""); await api.saveAssemblyStep({ ...draft, modelId, title: draft.title.trim(), instructions: draft.instructions.trim() }); setDraft(undefined); await refresh(); }
    catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)); }
  };
  const edit = (step: AssemblyStep) => setDraft({ id: step.id, title: step.title, instructions: step.instructions, sortOrder: step.sortOrder, bomItemIds: step.bomItemIds, photoIds: step.photoIds });
  const remove = async (step: AssemblyStep) => { if (!window.confirm(t("Delete this assembly step?"))) return; await api.deleteAssemblyStep(step.id); await refresh(); };
  const move = async (index: number, direction: -1 | 1) => {
    const step = steps[index]; const other = steps[index + direction]; if (!step || !other) return;
    const saveAt = (value: AssemblyStep, sortOrder: number) => api.saveAssemblyStep({ id: value.id, modelId, title: value.title, instructions: value.instructions, sortOrder, bomItemIds: value.bomItemIds, photoIds: value.photoIds });
    await saveAt(step, other.sortOrder); await saveAt(other, step.sortOrder); await refresh();
  };
  const toggle = (key: "bomItemIds" | "photoIds", id: string) => draft && setDraft({ ...draft, [key]: draft[key].includes(id) ? draft[key].filter((value) => value !== id) : [...draft[key], id] });
  return <Section title={t("Assembly guide")} count={steps.length} icon={<ListChecks size={16} />} open={steps.length > 0}>
    <div className="section-intro"><p className="quiet-copy">{t("Build an ordered guide and link each step to its parts and photos.")}</p><button className="button button--quiet" onClick={() => setDraft(draft ? undefined : emptyStep())}>{draft ? <X size={14} /> : <Plus size={14} />}{t(draft ? "Cancel" : "Add step")}</button></div>
    {error && <p className="form-error">{error}</p>}
    {draft && <form className="inline-editor" onSubmit={(event) => void save(event)}><label className="field field--compact inline-editor__wide"><span>{t("Step title")}</span><input autoFocus value={draft.title} maxLength={160} onChange={(event) => setDraft({ ...draft, title: event.target.value })} /></label><label className="field field--compact inline-editor__wide"><span>{t("Instructions")}</span><textarea value={draft.instructions} onChange={(event) => setDraft({ ...draft, instructions: event.target.value })} /></label>{items.length > 0 && <fieldset className="link-checklist"><legend>{t("Parts used")}</legend>{items.map((item) => <button type="button" className={draft.bomItemIds.includes(item.id) ? "is-selected" : ""} key={item.id} onClick={() => toggle("bomItemIds", item.id)}><span className="check-box">{draft.bomItemIds.includes(item.id) && <Check size={11} />}</span>{item.name}</button>)}</fieldset>}{photos.length > 0 && <fieldset className="link-checklist"><legend>{t("Linked photos")}</legend>{photos.map((photo) => <button type="button" className={draft.photoIds.includes(photo.id) ? "is-selected" : ""} key={photo.id} onClick={() => toggle("photoIds", photo.id)}><span className="check-box">{draft.photoIds.includes(photo.id) && <Check size={11} />}</span>{photo.caption || photo.originalFilename}</button>)}</fieldset>}<button className="button button--primary" disabled={!draft.title.trim()}><Save size={14} />{t(draft.id ? "Save step" : "Add step")}</button></form>}
    {steps.length ? <ol className="assembly-list">{steps.map((step, index) => <li key={step.id}><span className="assembly-list__number">{index + 1}</span><div><strong>{step.title}</strong>{step.instructions && <p>{step.instructions}</p>}<small>{pluralLinks(step.bomItemIds.length, step.photoIds.length)}</small>{step.photoIds.length > 0 && <div className="assembly-list__photos">{step.photoIds.map((id) => { const photo = photos.find((item) => item.id === id); return photo ? <span key={id}><PhotoImage photo={photo} /></span> : null; })}</div>}</div><div className="documentation-row__actions"><button className="icon-button icon-button--tiny" disabled={index === 0} onClick={() => void move(index, -1)}><ArrowUp size={13} /></button><button className="icon-button icon-button--tiny" disabled={index === steps.length - 1} onClick={() => void move(index, 1)}><ArrowDown size={13} /></button><button className="icon-button icon-button--tiny" onClick={() => edit(step)}><Pencil size={13} /></button><button className="icon-button icon-button--tiny" onClick={() => void remove(step)}><Trash2 size={13} /></button></div></li>)}</ol> : <p className="quiet-copy">{t("No assembly steps have been added yet.")}</p>}
  </Section>;
}

function pluralLinks(parts: number, photos: number) {
  return t("{parts} parts · {photos} photos", { parts, photos });
}

export function ProjectDocumentationSections({ modelId, assets }: { modelId: string; assets: Asset[] }) {
  const [enabled, setEnabled] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const element = rootRef.current;
    if (!element || typeof IntersectionObserver === "undefined") { setEnabled(true); return; }
    const observer = new IntersectionObserver((entries) => {
      if (entries.some((entry) => entry.isIntersecting)) {
        setEnabled(true);
        observer.disconnect();
      }
    }, { rootMargin: "800px" });
    observer.observe(element);
    return () => observer.disconnect();
  }, [modelId]);
  const { data = { photos: [], customValues: [], bomItems: [], assemblySteps: [] } } = useQuery({ queryKey: ["project-documentation", modelId], queryFn: () => api.projectDocumentation(modelId), enabled });
  const { data: definitions = [] } = useQuery({ queryKey: ["custom-fields"], queryFn: api.customFields, enabled, staleTime: Infinity });
  return <div ref={rootRef} className="project-documentation-sections">
    {enabled && <>
    <ProjectPhotos modelId={modelId} photos={data.photos} />
    <CustomFields modelId={modelId} definitions={definitions} values={data.customValues} />
    <BillOfMaterials modelId={modelId} assets={assets} items={data.bomItems} />
    <AssemblyGuide modelId={modelId} steps={data.assemblySteps} items={data.bomItems} photos={data.photos} />
    </>}
  </div>;
}
