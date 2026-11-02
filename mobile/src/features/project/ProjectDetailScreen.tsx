import { useQuery, useQueryClient } from "@tanstack/react-query";
import { open, save } from "@tauri-apps/plugin-dialog";
import { writeFile } from "@tauri-apps/plugin-fs";
import { openUrl } from "@tauri-apps/plugin-opener";
import QRCode from "qrcode";
import { lazy, Suspense, useCallback, useEffect, useRef, useState } from "react";
import {
  ArrowDown, ArrowLeft, ArrowUp, Box, Boxes, CalendarDays, Camera, Check, ChevronDown,
  ChevronRight, CircleAlert, CloudCheck, CloudDownload, CloudOff, Crosshair, ExternalLink, Eye,
  FileImage, Folder, Gauge, HardDrive, Heart, Layers3, ListChecks, LoaderCircle, Package, Pencil,
  Plus, Printer, QrCode, RotateCcw, Ruler, Save, Scale, Share2, Sparkles, Tag as TagIcon, Trash2,
  UploadCloud, Wifi, Wrench, X,
} from "lucide-react";
import { ErrorState } from "../../components/ErrorState";
import { PullToRefresh } from "../../components/PullToRefresh";
import { COLLECTION_COLORS } from "../../lib/colors";
import { validatedExternalUrl } from "../../lib/external";
import {
  formatArea, formatBytes, formatDate, formatDateTime, formatDuration, formatMoney,
  formatNumber, formatVolume,
} from "../../lib/format";
import { localizedError, t } from "../../lib/i18n";
import {
  applyOptimisticProjectChanges, optimisticProjectMutation,
  type OptimisticProjectChanges,
} from "../../lib/optimisticProject";
import {
  downloadOfflineProject, getOfflineProject, removeOfflineProject,
  type OfflineDownloadProgress,
} from "../../lib/offline";
import { projectQrPayload } from "../../lib/projectQr";
import type { LocalStlObject } from "../../lib/localStl";
import {
  cancelUpload, filenameFromFilePath, mutateJson, requestBlob, requestJson, uploadProjectFile,
  uploadProjectPhoto,
} from "../../lib/transport";
import type {
  AssemblyStep, Collection, Credential, CustomFieldDefinition, Material, ProjectBomItem,
  ProjectDetail, ProjectDocumentation, ProjectPhoto, RelatedProject, Tag, UploadProgress,
  CompanionAsset,
} from "../../types";
import { PhotoImage, PlateThumbnail, ProjectImage } from "./ProjectMedia";

const MobileModelViewer = lazy(() => import("../../components/MobileModelViewer").then((module) => ({ default: module.MobileModelViewer })));
const MobilePhotoRoom = lazy(() => import("../photo-room/MobilePhotoRoom").then((module) => ({ default: module.MobilePhotoRoom })));

type BomDraft = Pick<ProjectBomItem, "kind" | "name" | "quantity" | "unit" | "notes"> & { id?: string; linkedAssetId?: string; url?: string; sortOrder?: number };
type StepDraft = Pick<AssemblyStep, "title" | "instructions" | "bomItemIds" | "photoIds"> & { id?: string; sortOrder?: number };
type PhotoDraft = { file?: File; photo?: ProjectPhoto; caption: string; isCover: boolean };
type CostDraft = { assetId: string; materialId: string; plasticG: string; quantity: string; source: "manual" | "3mf" | "web" };

export function assetMimeType(extension: string) {
  switch (extension.toLowerCase()) {
    case "stl": return "model/stl";
    case "3mf": return "model/3mf";
    case "obj": return "model/obj";
    case "step": case "stp": return "model/step";
    default: return "application/octet-stream";
  }
}

function CustomValueControl({ field, value, busy, onSave }: { field: CustomFieldDefinition; value: string | number | boolean | null | undefined; busy: boolean; onSave: (value: string | number | boolean | null) => void }) {
  const serialized = value == null ? "" : String(value);
  const [draft, setDraft] = useState(serialized);
  useEffect(() => setDraft(serialized), [serialized]);
  const save = () => {
    if (!draft.trim()) return onSave(null);
    if (field.fieldType === "number") return onSave(Number(draft));
    if (field.fieldType === "boolean") return onSave(draft === "true");
    onSave(draft.trim());
  };
  return <div className="custom-value-editor"><label><span>{field.name}{field.unit ? ` · ${field.unit}` : ""}</span>{field.fieldType === "boolean" ? <select value={draft} onChange={(event) => setDraft(event.target.value)}><option value="">{t("Not set")}</option><option value="true">{t("Yes")}</option><option value="false">{t("No")}</option></select> : field.fieldType === "select" ? <select value={draft} onChange={(event) => setDraft(event.target.value)}><option value="">{t("Not set")}</option>{field.options.map((option) => <option key={option} value={option}>{option}</option>)}</select> : <input type={field.fieldType === "number" ? "number" : field.fieldType === "date" ? "date" : field.fieldType === "url" ? "url" : "text"} value={draft} step={field.fieldType === "number" ? "any" : undefined} onChange={(event) => setDraft(event.target.value)} />}</label><button aria-label={t("Save {name}", { name: field.name })} disabled={busy || draft === serialized || (field.fieldType === "number" && Boolean(draft) && !Number.isFinite(Number(draft)))} onClick={save}>{busy ? <LoaderCircle className="spin" /> : <Check />}</button></div>;
}

export function ProjectDetailScreen({ credential, id, canWrite: writesAllowed, onBack, onOpen }: { credential: Credential; id: string; canWrite: boolean; onBack: () => void; onOpen: (id: string) => void }) {
  const queryClient = useQueryClient();
  const photoInput = useRef<HTMLInputElement>(null);
  const [uploading, setUploading] = useState(false);
  const [uploadProgress, setUploadProgress] = useState<UploadProgress>();
  const [uploadError, setUploadError] = useState("");
  const [fileUploadId, setFileUploadId] = useState("");
  const [photoUploadId, setPhotoUploadId] = useState("");
  const [cancelingUpload, setCancelingUpload] = useState("");
  const [editingNotes, setEditingNotes] = useState(false);
  const [notesDraft, setNotesDraft] = useState("");
  const [savingProject, setSavingProject] = useState(false);
  const [collectionBusy, setCollectionBusy] = useState("");
  const [editError, setEditError] = useState("");
  const [editingName, setEditingName] = useState(false);
  const [nameDraft, setNameDraft] = useState("");
  const [editingTags, setEditingTags] = useState(false);
  const [tagDraft, setTagDraft] = useState<string[]>([]);
  const [newTagName, setNewTagName] = useState("");
  const [newTagColor, setNewTagColor] = useState("#ff5a36");
  const [creatingTag, setCreatingTag] = useState(false);
  const [tagEditor, setTagEditor] = useState<{ tag: Tag; name: string; color: string }>();
  const [tagEditorBusy, setTagEditorBusy] = useState(false);
  const [documentationBusy, setDocumentationBusy] = useState("");
  const [bomDraft, setBomDraft] = useState<BomDraft>();
  const [stepDraft, setStepDraft] = useState<StepDraft>();
  const [showViewer, setShowViewer] = useState(false);
  const [photoRoomOpen, setPhotoRoomOpen] = useState(false);
  const [viewerReset, setViewerReset] = useState(0);
  const [viewerAssetId, setViewerAssetId] = useState<string>();
  const [plateIndex, setPlateIndex] = useState<number>();
  const [viewerObjects, setViewerObjects] = useState<LocalStlObject[]>([]);
  const [selectedObjectId, setSelectedObjectId] = useState<number>();
  const [isolatedObjectId, setIsolatedObjectId] = useState<number>();
  const [focusObjectId, setFocusObjectId] = useState<number>();
  const [focusSignal, setFocusSignal] = useState(0);
  const [viewerObjectsOpen, setViewerObjectsOpen] = useState(false);
  const [photoDraft, setPhotoDraft] = useState<PhotoDraft>();
  const [photoProgress, setPhotoProgress] = useState<UploadProgress>();
  const [offlineProgress, setOfflineProgress] = useState<OfflineDownloadProgress>();
  const [offlineBusy, setOfflineBusy] = useState(false);
  const [workshopMode, setWorkshopMode] = useState(false);
  const [costDraft, setCostDraft] = useState<CostDraft>();
  const [shareMessage, setShareMessage] = useState("");
  const [shareOpen, setShareOpen] = useState(false);
  const [shareBusy, setShareBusy] = useState("");
  const [preparedShare, setPreparedShare] = useState<{ assetId: string; file: File }>();
  const [projectQr, setProjectQr] = useState("");
  const [projectQrBusy, setProjectQrBusy] = useState(false);
  const detail = useQuery({ queryKey: ["project", id], queryFn: () => requestJson<ProjectDetail>(credential, `/api/v1/projects/${encodeURIComponent(id)}`) });
  const documentation = useQuery({ queryKey: ["documentation", id], queryFn: () => requestJson<ProjectDocumentation>(credential, `/api/v1/projects/${encodeURIComponent(id)}/documentation`) });
  const offline = useQuery({ queryKey: ["offline-project", credential.serverId, id], queryFn: () => getOfflineProject(credential.serverId, id), staleTime: Infinity });
  const fields = useQuery({ queryKey: ["custom-fields"], queryFn: () => requestJson<CustomFieldDefinition[]>(credential, "/api/v1/custom-fields") });
  const collections = useQuery({ queryKey: ["collections"], queryFn: () => requestJson<Collection[]>(credential, "/api/v1/collections") });
  const tags = useQuery({ queryKey: ["tags"], queryFn: () => requestJson<Tag[]>(credential, "/api/v1/tags") });
  const materials = useQuery({ queryKey: ["materials"], queryFn: () => requestJson<Material[]>(credential, "/api/v1/materials") });
  const related = useQuery({ queryKey: ["related", id], queryFn: () => requestJson<RelatedProject[]>(credential, `/api/v1/projects/${encodeURIComponent(id)}/related`) });
  const project = detail.data ?? offline.data?.detail;
  const docs = documentation.data ?? offline.data?.documentation;
  const canWrite = writesAllowed && Boolean(detail.data);
  const defaultViewerAsset = project?.assets.find((asset) => asset.id === project.primaryAssetId && !asset.missing)
    ?? project?.assets.find((asset) => ["stl", "3mf", "obj", "step", "stp"].includes(asset.extension.toLowerCase()) && !asset.missing);
  const viewerAsset = project?.assets.find((asset) => asset.id === viewerAssetId && !asset.missing) ?? defaultViewerAsset;
  const viewerPlates = viewerAsset?.metadata.threeMf?.plates ?? [];
  const offlineViewerAssetId = offline.data?.viewerAssetId ?? offline.data?.detail.primaryAssetId;
  const offlineViewerPayload = viewerAsset?.id === offlineViewerAssetId ? offline.data?.viewerMesh : undefined;
  const offlinePhotoMap = new Map(offline.data?.photoThumbnails.map((item) => [item.photoId, item.blob]) ?? []);
  const handleViewerStatus = useCallback((_status: "ready" | "error", _message?: string) => undefined, []);
  useEffect(() => {
    setViewerObjects([]);
    setSelectedObjectId(undefined);
    setIsolatedObjectId(undefined);
    setFocusObjectId(undefined);
    setViewerObjectsOpen(false);
  }, [plateIndex, showViewer, viewerAsset?.id]);
  const selectedObject = viewerObjects.find((object) => object.id === selectedObjectId);
  const selectViewerObject = (objectId?: number) => {
    if (!objectId) {
      setSelectedObjectId(undefined);
      setFocusObjectId(undefined);
      if (isolatedObjectId) {
        setIsolatedObjectId(undefined);
        setViewerReset((value) => value + 1);
      }
      return;
    }
    setSelectedObjectId(objectId);
  };
  const focusViewerObject = (objectId: number) => {
    setSelectedObjectId(objectId);
    setFocusObjectId(objectId);
    setFocusSignal((value) => value + 1);
  };
  const isolateViewerObject = (objectId: number) => {
    setSelectedObjectId(objectId);
    setIsolatedObjectId(objectId);
    setFocusObjectId(objectId);
    setFocusSignal((value) => value + 1);
  };
  const showAllViewerObjects = () => {
    setIsolatedObjectId(undefined);
    setFocusObjectId(undefined);
    setViewerReset((value) => value + 1);
  };
  const fieldMap = new Map((fields.data ?? []).map((field) => [field.id, field]));
  const saveProject = async (changes: OptimisticProjectChanges) => {
    if (!project) return false;
    setSavingProject(true); setEditError("");
    try {
      const optimistic = applyOptimisticProjectChanges(project, changes, tags.data ?? project.tags);
      await optimisticProjectMutation(queryClient, id, optimistic, () =>
        mutateJson<ProjectDetail>(credential, `/api/v1/projects/${encodeURIComponent(id)}`, "PATCH", {
          ...changes,
          ifUnmodifiedSince: project.revision,
        }));
      await queryClient.invalidateQueries({ queryKey: ["projects"] });
      return true;
    } catch (reason) {
      setEditError(localizedError(reason));
      return false;
    } finally { setSavingProject(false); }
  };
  const beginNotes = () => { setNotesDraft(project?.notes ?? ""); setEditingNotes(true); setEditError(""); };
  const saveNotes = async () => { setEditingNotes(false); if (!await saveProject({ notes: notesDraft })) setEditingNotes(true); };
  const beginRename = () => { setNameDraft(project?.displayName ?? ""); setEditingName(true); setEditError(""); };
  const saveName = async () => { setEditingName(false); if (!await saveProject({ displayName: nameDraft.trim() })) setEditingName(true); };
  const beginTags = () => { setTagDraft(project?.tags.map((tag) => tag.id) ?? []); setEditingTags(true); setEditError(""); };
  const saveTags = async () => { setEditingTags(false); if (!await saveProject({ tagIds: tagDraft })) setEditingTags(true); };
  const createTag = async () => {
    if (!newTagName.trim()) return;
    setCreatingTag(true); setEditError("");
    try {
      const created = await mutateJson<Tag>(credential, "/api/v1/tags", "POST", { name: newTagName.trim(), color: newTagColor });
      queryClient.setQueryData<Tag[]>(["tags"], (current = []) => [...current, created].sort((left, right) => left.name.localeCompare(right.name)));
      setTagDraft((current) => [...current, created.id]);
      setNewTagName("");
    } catch (reason) { setEditError(localizedError(reason)); }
    finally { setCreatingTag(false); }
  };
  const saveManagedTag = async () => {
    if (!tagEditor?.name.trim()) return;
    setTagEditorBusy(true); setEditError("");
    try {
      await mutateJson<Tag>(credential, `/api/v1/tags/${encodeURIComponent(tagEditor.tag.id)}`, "PATCH", {
        name: tagEditor.name.trim(),
        color: tagEditor.color,
      });
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["tags"] }),
        queryClient.invalidateQueries({ queryKey: ["projects"] }),
        detail.refetch(),
      ]);
      setTagEditor(undefined);
    } catch (reason) { setEditError(localizedError(reason)); }
    finally { setTagEditorBusy(false); }
  };
  const deleteManagedTag = async () => {
    if (!tagEditor || !window.confirm(t("Delete “{name}”? It will be removed from {count} projects.", { name: tagEditor.tag.name, count: tagEditor.tag.modelCount }))) return;
    setTagEditorBusy(true); setEditError("");
    try {
      await mutateJson<void>(credential, `/api/v1/tags/${encodeURIComponent(tagEditor.tag.id)}`, "DELETE");
      setTagDraft((current) => current.filter((tagId) => tagId !== tagEditor.tag.id));
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["tags"] }),
        queryClient.invalidateQueries({ queryKey: ["projects"] }),
        detail.refetch(),
      ]);
      setTagEditor(undefined);
    } catch (reason) { setEditError(localizedError(reason)); }
    finally { setTagEditorBusy(false); }
  };
  const saveDocumentation = async (path: string, method: "PUT" | "POST" | "PATCH" | "DELETE", body: Record<string, unknown>, busyKey: string) => {
    if (!docs) return false;
    setDocumentationBusy(busyKey); setEditError("");
    try {
      const updated = await mutateJson<ProjectDocumentation>(credential, path, method, { ...body, ifUnmodifiedSince: docs.revision });
      queryClient.setQueryData(["documentation", id], updated);
      await Promise.all([detail.refetch(), queryClient.invalidateQueries({ queryKey: ["projects"] })]);
      return true;
    } catch (reason) {
      setEditError(localizedError(reason));
      return false;
    } finally { setDocumentationBusy(""); }
  };
  const saveCustomValue = (fieldId: string, value: string | number | boolean | null) => saveDocumentation(`/api/v1/projects/${encodeURIComponent(id)}/custom-fields/${encodeURIComponent(fieldId)}`, "PUT", { value }, `field:${fieldId}`);
  const editBomItem = (item?: ProjectBomItem) => setBomDraft(item ? { id: item.id, kind: item.kind, linkedAssetId: item.linkedAssetId, name: item.name, quantity: item.quantity, unit: item.unit, url: item.url, notes: item.notes, sortOrder: item.sortOrder } : { kind: "purchased", name: "", quantity: 1, unit: "pcs", notes: "" });
  const saveBomItem = async () => {
    if (!bomDraft) return;
    if (await saveDocumentation(`/api/v1/projects/${encodeURIComponent(id)}/bom`, "POST", bomDraft, "bom")) setBomDraft(undefined);
  };
  const removeBomItem = async (item: ProjectBomItem) => {
    if (!window.confirm(t("Delete “{name}” from the bill of materials?", { name: item.name }))) return;
    await saveDocumentation(`/api/v1/projects/${encodeURIComponent(id)}/bom/${encodeURIComponent(item.id)}`, "DELETE", {}, `bom:${item.id}`);
  };
  const moveBomItem = async (item: ProjectBomItem, direction: -1 | 1) => {
    if (!docs) return;
    const ordered = [...docs.bomItems].sort((left, right) => left.sortOrder - right.sortOrder);
    const index = ordered.findIndex((candidate) => candidate.id === item.id);
    const target = ordered[index + direction];
    if (!target) return;
    const outer = ordered[index + direction * 2];
    const sortOrder = outer == null ? target.sortOrder + direction : (target.sortOrder + outer.sortOrder) / 2;
    await saveDocumentation(`/api/v1/projects/${encodeURIComponent(id)}/bom`, "POST", { ...item, sortOrder }, `bom:${item.id}`);
  };
  const editAssemblyStep = (step?: AssemblyStep) => setStepDraft(step ? { id: step.id, title: step.title, instructions: step.instructions, sortOrder: step.sortOrder, bomItemIds: [...step.bomItemIds], photoIds: [...step.photoIds] } : { title: "", instructions: "", bomItemIds: [], photoIds: [] });
  const saveStep = async () => {
    if (!stepDraft) return;
    if (await saveDocumentation(`/api/v1/projects/${encodeURIComponent(id)}/assembly-steps`, "POST", stepDraft, "step")) setStepDraft(undefined);
  };
  const removeStep = async (step: AssemblyStep) => {
    if (!window.confirm(t("Delete “{name}”?", { name: step.title || t("this assembly step") }))) return;
    await saveDocumentation(`/api/v1/projects/${encodeURIComponent(id)}/assembly-steps/${encodeURIComponent(step.id)}`, "DELETE", {}, `step:${step.id}`);
  };
  const moveStep = async (step: AssemblyStep, direction: -1 | 1) => {
    if (!docs) return;
    const ordered = [...docs.assemblySteps].sort((left, right) => left.sortOrder - right.sortOrder);
    const index = ordered.findIndex((candidate) => candidate.id === step.id);
    const target = ordered[index + direction];
    if (!target) return;
    const outer = ordered[index + direction * 2];
    const sortOrder = outer == null ? target.sortOrder + direction : (target.sortOrder + outer.sortOrder) / 2;
    await saveDocumentation(`/api/v1/projects/${encodeURIComponent(id)}/assembly-steps`, "POST", { ...step, sortOrder }, `step:${step.id}`);
  };
  const toggleCollection = async (collection: Collection) => {
    if (!project || collection.smart) return;
    const selected = project.collectionIds.includes(collection.id);
    const previousCollections = queryClient.getQueryData<Collection[]>(["collections"]);
    setCollectionBusy(collection.id); setEditError("");
    queryClient.setQueryData<Collection[]>(["collections"], (current) => current?.map((item) => item.id === collection.id
      ? { ...item, modelCount: Math.max(0, item.modelCount + (selected ? -1 : 1)) }
      : item));
    try {
      await optimisticProjectMutation(
        queryClient,
        id,
        {
          ...project,
          collectionIds: selected
            ? project.collectionIds.filter((collectionId) => collectionId !== collection.id)
            : [...project.collectionIds, collection.id],
        },
        () => mutateJson<ProjectDetail>(credential, `/api/v1/collections/${encodeURIComponent(collection.id)}/projects/${encodeURIComponent(id)}`, selected ? "DELETE" : "PUT"),
      );
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["collections"] }),
        queryClient.invalidateQueries({ queryKey: ["projects"] })
      ]);
    } catch (reason) {
      queryClient.setQueryData(["collections"], previousCollections);
      setEditError(localizedError(reason));
    }
    finally { setCollectionBusy(""); }
  };
  const addFile = async () => {
    setUploadError("");
    try {
      const selected = await open({ multiple: false, directory: false, title: t("Choose a 3D file"), filters: [{ name: t("3D files"), extensions: ["stl", "3mf", "obj", "step", "stp"] }] });
      if (!selected || Array.isArray(selected)) return;
      const filename = filenameFromFilePath(selected);
      setUploading(true); setUploadProgress(undefined);
      const uploadId = crypto.randomUUID();
      setFileUploadId(uploadId);
      await uploadProjectFile(credential, selected, filename, { projectId: id }, setUploadProgress, uploadId);
      await Promise.all([
        detail.refetch(),
        queryClient.invalidateQueries({ queryKey: ["projects"] }),
        queryClient.invalidateQueries({ queryKey: ["status"] })
      ]);
    } catch (reason) {
      const message = reason instanceof Error ? reason.message : String(reason);
      if (message !== "Upload canceled") setUploadError(localizedError(reason));
    } finally { setUploading(false); setFileUploadId(""); setCancelingUpload(""); }
  };
  const choosePhoto = (capture: boolean) => {
    if (!photoInput.current) return;
    if (capture) photoInput.current.setAttribute("capture", "environment");
    else photoInput.current.removeAttribute("capture");
    photoInput.current.click();
  };
  const savePhoto = async () => {
    if (!docs || !photoDraft) return;
    if (photoDraft.file) {
      setDocumentationBusy("photo-upload"); setEditError(""); setPhotoProgress(undefined);
      const uploadId = crypto.randomUUID();
      setPhotoUploadId(uploadId);
      try {
        let updated = await uploadProjectPhoto(credential, photoDraft.file, id, photoDraft.caption, docs.revision, setPhotoProgress, uploadId);
        const added = updated.photos.find((photo) => !docs.photos.some((existing) => existing.id === photo.id));
        if (photoDraft.isCover && added && !added.isCover) {
          updated = await mutateJson<ProjectDocumentation>(credential, `/api/v1/projects/${encodeURIComponent(id)}/photos/${encodeURIComponent(added.id)}`, "PATCH", {
            caption: added.caption,
            sortOrder: added.sortOrder,
            isCover: true,
            ifUnmodifiedSince: updated.revision,
          });
        }
        queryClient.setQueryData(["documentation", id], updated);
        await Promise.all([detail.refetch(), queryClient.invalidateQueries({ queryKey: ["projects"] })]);
        setPhotoDraft(undefined);
      } catch (reason) {
        const message = reason instanceof Error ? reason.message : String(reason);
        if (message !== "Upload canceled") setEditError(localizedError(reason));
      } finally { setDocumentationBusy(""); setPhotoUploadId(""); setCancelingUpload(""); }
      return;
    }
    if (!photoDraft.photo) return;
    const saved = await saveDocumentation(
      `/api/v1/projects/${encodeURIComponent(id)}/photos/${encodeURIComponent(photoDraft.photo.id)}`,
      "PATCH",
      { caption: photoDraft.caption, sortOrder: photoDraft.photo.sortOrder, isCover: photoDraft.isCover },
      `photo:${photoDraft.photo.id}`,
    );
    if (saved) setPhotoDraft(undefined);
  };
  const cancelActiveUpload = async (uploadId: string) => {
    if (!uploadId || cancelingUpload) return;
    setCancelingUpload(uploadId);
    try {
      const canceled = await cancelUpload(uploadId);
      if (!canceled) {
        const message = t("The upload is still starting or has already finished. Try again if it remains active.");
        if (uploadId === fileUploadId) setUploadError(message); else setEditError(message);
        setCancelingUpload("");
      }
    }
    catch (reason) {
      const message = localizedError(reason);
      if (uploadId === fileUploadId) setUploadError(message); else setEditError(message);
      setCancelingUpload("");
    }
  };
  const removePhoto = async (photo: ProjectPhoto) => {
    if (!window.confirm(t("Delete “{name}”?", { name: photo.caption || photo.originalFilename }))) return;
    await saveDocumentation(`/api/v1/projects/${encodeURIComponent(id)}/photos/${encodeURIComponent(photo.id)}`, "DELETE", {}, `photo:${photo.id}`);
  };
  const movePhoto = async (photo: ProjectPhoto, direction: -1 | 1) => {
    if (!docs) return;
    const ordered = [...docs.photos].sort((left, right) => left.sortOrder - right.sortOrder);
    const index = ordered.findIndex((candidate) => candidate.id === photo.id);
    const target = ordered[index + direction];
    if (!target) return;
    const before = direction < 0 ? ordered[index - 2]?.sortOrder : photo.sortOrder;
    const after = direction < 0 ? target.sortOrder : ordered[index + 2]?.sortOrder;
    const sortOrder = direction < 0
      ? before == null ? target.sortOrder - 1 : (before + target.sortOrder) / 2
      : after == null ? target.sortOrder + 1 : (target.sortOrder + after) / 2;
    await saveDocumentation(
      `/api/v1/projects/${encodeURIComponent(id)}/photos/${encodeURIComponent(photo.id)}`,
      "PATCH",
      { caption: photo.caption, sortOrder, isCover: photo.isCover },
      `photo:${photo.id}`,
    );
  };
  const toggleOffline = async () => {
    if (!project) return;
    setOfflineBusy(true); setEditError(""); setOfflineProgress(undefined);
    try {
      if (offline.data) await removeOfflineProject(credential.serverId, id);
      else await downloadOfflineProject(credential, id, setOfflineProgress);
      await offline.refetch();
    } catch (reason) { setEditError(localizedError(reason)); }
    finally { setOfflineBusy(false); setOfflineProgress(undefined); }
  };
  const beginCost = () => {
    const estimate = project?.estimate;
    const suggestedAsset = viewerAsset?.id ?? "";
    const suggestedWeight = estimate?.plasticG ?? viewerAsset?.metadata.filamentGrams ?? project?.webSource?.filamentGrams ?? 0;
    setCostDraft({
      assetId: estimate?.assetId ?? suggestedAsset,
      materialId: estimate?.materialId ?? "",
      plasticG: suggestedWeight ? String(suggestedWeight) : "",
      quantity: String(estimate?.quantity ?? 1),
      source: estimate?.source === "3mf" || estimate?.source === "web" ? estimate.source : "manual",
    });
  };
  const saveCost = async () => {
    if (!project || !costDraft) return;
    setSavingProject(true); setEditError("");
    try {
      const updated = await mutateJson<ProjectDetail>(credential, `/api/v1/projects/${encodeURIComponent(id)}/estimate`, "PUT", {
        assetId: costDraft.assetId || undefined,
        materialId: costDraft.materialId || undefined,
        plasticG: Number(costDraft.plasticG),
        quantity: Number(costDraft.quantity),
        source: costDraft.source,
        ifUnmodifiedSince: project.revision,
      });
      queryClient.setQueryData(["project", id], updated);
      setCostDraft(undefined);
    } catch (reason) { setEditError(localizedError(reason)); }
    finally { setSavingProject(false); }
  };
  const fetchAssetFile = async (asset: CompanionAsset) => {
    const blob = await requestBlob(credential, `/api/v1/assets/${encodeURIComponent(asset.id)}/content`, assetMimeType(asset.extension));
    return new File([blob], asset.filename, { type: assetMimeType(asset.extension), lastModified: Date.parse(asset.modifiedAt) || Date.now() });
  };
  const saveAsset = async (asset: CompanionAsset, file?: File) => {
    const destination = await save({
      defaultPath: asset.filename,
      filters: [{ name: asset.extension.toUpperCase() || t("Project file"), extensions: [asset.extension.toLowerCase()] }],
    });
    if (!destination) return false;
    const payload = file ?? await fetchAssetFile(asset);
    await writeFile(destination, new Uint8Array(await payload.arrayBuffer()));
    setShareMessage(t("Saved {name} to Files.", { name: asset.filename }));
    return true;
  };
  const shareAsset = async (asset: CompanionAsset) => {
    setShareMessage(""); setEditError("");
    try {
      if (preparedShare?.assetId !== asset.id) {
        setShareBusy(`share:${asset.id}`);
        const file = await fetchAssetFile(asset);
        setPreparedShare({ assetId: asset.id, file });
        setShareMessage(t("File ready. Tap Share again to choose where to send it."));
        return;
      }
      const file = preparedShare.file;
      const shareData: ShareData = { title: project?.displayName, files: [file] };
      if (navigator.share && (!navigator.canShare || navigator.canShare(shareData))) {
        await navigator.share(shareData);
        setShareOpen(false);
        setPreparedShare(undefined);
      } else {
        if (await saveAsset(asset, file)) {
          setShareOpen(false);
          setPreparedShare(undefined);
        }
      }
    } catch (reason) {
      if (reason instanceof DOMException && reason.name === "AbortError") return;
      setEditError(localizedError(reason));
    } finally { setShareBusy(""); }
  };
  const downloadAsset = async (asset: CompanionAsset) => {
    setShareMessage(""); setEditError(""); setShareBusy(`save:${asset.id}`);
    try { await saveAsset(asset); }
    catch (reason) { setEditError(localizedError(reason)); }
    finally { setShareBusy(""); }
  };
  const openSource = async () => {
    if (!project?.webSource) return;
    setEditError("");
    try { await openUrl(validatedExternalUrl(project.webSource.canonicalUrl).toString()); }
    catch (reason) { setEditError(localizedError(reason)); }
  };
  const showProjectQr = async () => {
    setProjectQrBusy(true); setEditError("");
    try {
      setProjectQr(await QRCode.toDataURL(projectQrPayload(credential.serverId, id), {
        width: 720,
        margin: 3,
        color: { dark: "#191916", light: "#fffefb" },
        errorCorrectionLevel: "M",
      }));
    } catch (reason) { setEditError(localizedError(reason)); }
    finally { setProjectQrBusy(false); }
  };
  if (detail.isLoading && offline.isLoading) return <main className="detail-screen"><button className="round-button" aria-label={t("Back to library")} onClick={onBack}><ArrowLeft /></button><div className="center-state" role="status" aria-label={t("Loading project")}><LoaderCircle className="spin" /></div></main>;
  if (!project) return <main className="detail-screen"><button className="round-button" aria-label={t("Back to library")} onClick={onBack}><ArrowLeft /></button><ErrorState error={detail.error} retrying={detail.isFetching} onRetry={() => void detail.refetch()} /></main>;
  const dimensions = project.dimensionsMm ?? project.assets.find((asset) => asset.metadata.dimensionsMm)?.metadata.dimensionsMm;
  const totalBytes = project.assets.reduce((total, asset) => total + asset.byteSize, 0);
  const projectFormat = (project.primaryExtension || "PROJECT").toUpperCase();
  if (workshopMode && docs) return <WorkshopScreen credential={credential} project={project} documentation={docs} offlinePhotos={offlinePhotoMap} onBack={() => setWorkshopMode(false)} />;
  if (photoRoomOpen && viewerAsset) return <Suspense fallback={<div className="center-state" role="status"><LoaderCircle className="spin" /></div>}><MobilePhotoRoom
    title={project.displayName}
    credential={detail.data ? credential : undefined}
    assetId={viewerAsset.id}
    revision={viewerAsset.modifiedAt}
    plateIndex={plateIndex}
    offlinePayload={offlineViewerPayload}
    projectId={project.id}
    documentationRevision={docs?.revision}
    canSaveToProject={canWrite}
    onClose={() => setPhotoRoomOpen(false)}
  /></Suspense>;
  if (showViewer && viewerAsset) return <main className="fullscreen-viewer">
    <Suspense fallback={<div className="viewer-state"><LoaderCircle className="spin" /><span>{t("Opening 3D viewer…")}</span></div>}><MobileModelViewer credential={credential} assetId={viewerAsset.id} revision={viewerAsset.modifiedAt} plateIndex={plateIndex} offlinePayload={offlineViewerPayload} resetSignal={viewerReset} selectedObjectId={selectedObjectId} isolatedObjectId={isolatedObjectId} focusObjectId={focusObjectId} focusSignal={focusSignal} onObjectSelect={selectViewerObject} onObjects={setViewerObjects} onStatus={handleViewerStatus} /></Suspense>
    <div className="fullscreen-viewer__toolbar"><button className="round-button fullscreen-viewer__back" aria-label={t("Back to project")} onClick={() => setShowViewer(false)}><ArrowLeft /></button><div className="fullscreen-viewer__toolbar-actions"><button className="round-button" aria-label={t("Open Photo Room")} onClick={() => setPhotoRoomOpen(true)}><Sparkles /></button><button className="round-button fullscreen-viewer__reset" aria-label={t("Reset 3D view")} onClick={() => setViewerReset((value) => value + 1)}><RotateCcw /></button></div></div>
    {viewerObjects.length > 0 && <button className="project-viewer-objects-button" aria-label={t("Open object list")} onClick={() => setViewerObjectsOpen(true)}><Layers3 /><span>{viewerObjects.length === 1 ? t("1 object") : t("{count} objects", { count: viewerObjects.length })}</span></button>}
    {selectedObject && <section className="local-object-inspector project-object-inspector" aria-label={t("Selected object details")}>
      <header><div><span>{t("OBJECT {current} OF {count}", { current: selectedObject.id, count: viewerObjects.length })}</span><strong>{selectedObject.name || t("Object {number}", { number: selectedObject.id })}</strong></div><button className="round-button" aria-label={t("Clear object selection")} onClick={() => selectViewerObject(undefined)}><X /></button></header>
      <div className="local-object-inspector__metrics"><div><span>{t("X · WIDTH")}</span><strong>{formatNumber(selectedObject.dimensions[0])} mm</strong></div><div><span>{t("Y · DEPTH")}</span><strong>{formatNumber(selectedObject.dimensions[1])} mm</strong></div><div><span>{t("Z · HEIGHT")}</span><strong>{formatNumber(selectedObject.dimensions[2])} mm</strong></div></div>
      <p>{t("{triangles} triangles · Base Z {z} mm", { triangles: formatNumber(selectedObject.triangleCount), z: formatNumber(selectedObject.bounds[2]) })}</p>
      <div className="local-object-inspector__actions"><button className="button button--quiet" onClick={() => focusViewerObject(selectedObject.id)}><Crosshair />{t("Fit to view")}</button>{isolatedObjectId ? <button className="button button--primary" onClick={showAllViewerObjects}><Layers3 />{t("Show all")}</button> : <button className="button button--primary" onClick={() => isolateViewerObject(selectedObject.id)}><Eye />{t("Inspect alone")}</button>}</div>
    </section>}
    {viewerPlates.length > 1 && detail.data && <label className="fullscreen-viewer__plate"><Layers3 /><select value={plateIndex ?? ""} onChange={(event) => setPlateIndex(event.target.value ? Number(event.target.value) : undefined)} aria-label={t("Plate")}><option value="">{t("All plates")}</option>{viewerPlates.map((plate) => <option key={plate.index} value={plate.index}>{t("Plate {number}", { number: plate.index })}</option>)}</select></label>}
    {!selectedObject && <div className="viewer-hint viewer-hint--fullscreen">{viewerObjects.length ? t("Tap an object · Drag to rotate") : t("Drag to rotate · Pinch to zoom")}</div>}
    {viewerObjectsOpen && <div className="sheet-scrim" role="presentation"><section className="mobile-sheet mobile-sheet--tall local-objects-sheet" role="dialog" aria-modal="true" aria-labelledby="plate-objects-title"><header><div><span className="eyebrow">{t("PLATE OBJECTS")}</span><h2 id="plate-objects-title">{t("Inspect individual objects")}</h2></div><button className="round-button" aria-label={t("Close")} onClick={() => setViewerObjectsOpen(false)}><X /></button></header><p>{t("Select an object to inspect its dimensions and placement.")}</p><div className="local-objects-list">{viewerObjects.map((object) => <button key={object.id} className={selectedObjectId === object.id ? "is-selected" : ""} aria-pressed={selectedObjectId === object.id} onClick={() => { selectViewerObject(object.id); setViewerObjectsOpen(false); }}><span><Box /></span><div><strong>{object.name || t("Object {number}", { number: object.id })}</strong><small>{object.dimensions.map((value) => `${formatNumber(value)} mm`).join(" × ")}</small><small>{t("{count} triangles", { count: formatNumber(object.triangleCount) })}</small></div>{selectedObjectId === object.id && <Check />}</button>)}</div></section></div>}
  </main>;
  return <main className="detail-screen">
    <PullToRefresh onRefresh={() => Promise.all([detail.refetch(), documentation.refetch(), fields.refetch(), collections.refetch(), tags.refetch()])} />
    <section className="detail-stage">
      <ProjectImage credential={credential} projectId={id} revision={project.revision} className="project-image--detail" offlineBlob={offline.data?.cover} detailFallback canOpenViewer={Boolean(viewerAsset)} />
      <div className="detail-stage__chrome">
        <button className="round-button detail-back" aria-label={t("Back to library")} onClick={onBack}><ArrowLeft /></button>
        <div className="detail-stage__actions"><button className="round-button" aria-label={t("Show project QR code")} disabled={projectQrBusy} onClick={() => void showProjectQr()}>{projectQrBusy ? <LoaderCircle className="spin" /> : <QrCode />}</button><button className="round-button" aria-label={t("Share project files")} disabled={!detail.data || !project.assets.some((asset) => !asset.missing)} onClick={() => { setShareMessage(""); setEditError(""); setPreparedShare(undefined); setShareOpen(true); }}><Share2 /></button>{canWrite ? <button className={`detail-favorite ${project.favorite ? "is-active" : ""}`} disabled={savingProject} aria-label={t(project.favorite ? "Remove from favorites" : "Add to favorites")} onClick={() => void saveProject({ favorite: !project.favorite })}>{savingProject ? <LoaderCircle className="spin" /> : <Heart size={18} fill={project.favorite ? "currentColor" : "none"} />}</button> : project.favorite ? <span className="detail-favorite is-active" role="img" aria-label={t("Favorite project")}><Heart size={18} fill="currentColor" /></span> : null}</div>
      </div>
      {viewerAsset && <div className="detail-stage__model-actions"><button className="viewer-open-button" disabled={!detail.data && !offlineViewerPayload} onClick={() => setShowViewer(true)}><span className="viewer-open-button__icon"><Box /></span><span>{t("View in 3D")}</span><ChevronRight /></button><button className="photo-room-open-button" disabled={!detail.data && !offlineViewerPayload} onClick={() => setPhotoRoomOpen(true)}><Sparkles /><span>{t("Photo Room")}</span></button></div>}
    </section>
    <article className="detail-content">
      <input ref={photoInput} className="visually-hidden" type="file" accept="image/jpeg,image/png,image/webp" onChange={(event) => { const file = event.target.files?.[0]; if (file) setPhotoDraft({ file, caption: "", isCover: !docs?.photos.length }); event.target.value = ""; }} />
      <header className="detail-intro">
        <div className="detail-kicker"><span>{projectFormat}</span><span>{project.assetCount} {t(project.assetCount === 1 ? "file" : "files")}</span></div>
        <div className="detail-title"><h1>{project.displayName}</h1>{canWrite && <button className="inline-icon-button" aria-label={t("Rename project")} onClick={beginRename}><Pencil /></button>}</div>
        <p className="detail-location"><Folder size={14} /><span>{project.rootName}</span><ChevronRight size={12} /><span>{project.folderName}</span></p>
      </header>
      <section className="detail-summary" aria-label={t("Project summary")}>
        <div className="detail-summary__item"><span className="detail-summary__icon">{dimensions ? <Ruler /> : <Box />}</span><div><span>{dimensions ? t("Dimensions") : t("Format")}</span><strong>{dimensions ? `${dimensions.map((value) => Math.round(value)).join(" × ")} mm` : projectFormat}</strong></div></div>
        <div className="detail-summary__item"><span className="detail-summary__icon"><CalendarDays /></span><div><span>{t("Modified")}</span><strong>{formatDate(project.modifiedAt)}</strong></div></div>
        <div className="detail-summary__item"><span className="detail-summary__icon"><HardDrive /></span><div><span>{t("Project size")}</span><strong>{formatBytes(totalBytes)}</strong></div></div>
      </section>
      {(canWrite || project.tags.length > 0) && <div className="detail-tags-inline"><span className="detail-tags-inline__icon"><TagIcon /></span><div className="tag-row">{project.tags.map((tag) => <span key={tag.id} style={{ "--tag-color": tag.color } as React.CSSProperties}><i />{tag.name}</span>)}</div>{canWrite && <button className="detail-tags-inline__edit" onClick={beginTags}>{project.tags.length ? <Pencil /> : <Plus />}<span>{project.tags.length ? t("Edit") : t("Add tags")}</span></button>}</div>}
      <section className={`offline-card ${offline.data ? "is-saved" : ""}`}><span>{offline.data ? <CloudCheck /> : <CloudDownload />}</span><div><strong>{offline.data ? t("Available offline") : t("Online only")}</strong><small>{offline.data ? `${formatBytes(offline.data.manifest.byteSize)} · ${t("Synced")} ${formatDateTime(offline.data.manifest.syncedAt)}${offline.data.manifest.revision !== project.revision ? ` · ${t("Update available")}` : ""}` : t("Save details, photos, and the 3D view to this device.")}</small>{offlineProgress && <div className="upload-progress"><span style={{ width: `${offlineProgress.completed / offlineProgress.total * 100}%` }} /></div>}</div><button disabled={offlineBusy || (!detail.data && Boolean(offline.data))} onClick={() => void toggleOffline()}>{offlineBusy ? <LoaderCircle className="spin" /> : offline.data ? t("Remove") : t("Download")}</button></section>
      {editError && <div className="error-banner" role="alert"><CircleAlert size={16} />{editError}</div>}
      {shareMessage && <div className="success-banner" role="status"><Check />{shareMessage}</div>}
      {(canWrite || project.notes) && <DetailSection title={t("Notes")} icon={<ListChecks />}>{editingNotes ? <div className="notes-editor"><textarea value={notesDraft} maxLength={100000} autoFocus onChange={(event) => setNotesDraft(event.target.value)} placeholder={t("Add build notes, settings, or reminders…")} /><div><button className="button button--quiet" disabled={savingProject} onClick={() => setEditingNotes(false)}><X />{t("Cancel")}</button><button className="button button--primary" disabled={savingProject} onClick={() => void saveNotes()}>{savingProject ? <LoaderCircle className="spin" /> : <Check />}{t("Save notes")}</button></div></div> : <div className="notes-view"><p className={project.notes ? "prose" : "prose prose--empty"}>{project.notes || t("No notes yet.")}</p>{canWrite && <button className="inline-edit" onClick={beginNotes}><Pencil />{project.notes ? t("Edit notes") : t("Add notes")}</button>}</div>}</DetailSection>}
      {(canWrite || project.collectionIds.length > 0) && <DetailSection title={t("Collections")} icon={<Boxes />} count={project.collectionIds.length}><div className="project-collections">{collections.isLoading ? <LoaderCircle className="spin" /> : collections.data?.length ? collections.data.map((collection) => { const selected = project.collectionIds.includes(collection.id); return <button key={collection.id} className={selected ? "is-selected" : ""} disabled={!canWrite || collection.smart || Boolean(collectionBusy)} onClick={() => void toggleCollection(collection)} style={{ "--collection-color": collection.color } as React.CSSProperties}><i /> <span><strong>{collection.name}</strong><small>{collection.smart ? t("Smart collection") : selected ? t("Included") : t("Tap to include")}</small></span>{collectionBusy === collection.id ? <LoaderCircle className="spin" /> : selected ? <Check /> : <Plus />}</button>; }) : <p className="prose prose--empty">{t("Create a collection from the Collections tab, then add this project here.")}</p>}</div></DetailSection>}
      {project.assets.some((asset) => asset.metadata.threeMf) && <DetailSection title={t("Print setup")} icon={<Printer />}><div className="print-profiles">{project.assets.filter((asset) => asset.metadata.threeMf).map((asset) => { const profile = asset.metadata.threeMf!; return <article key={asset.id}><header><div><strong>{asset.filename}</strong><small>{[profile.slicer, profile.slicerVersion].filter(Boolean).join(" · ") || t("3MF project")}</small></div><button disabled={!detail.data && asset.id !== offlineViewerAssetId} onClick={() => { setViewerAssetId(asset.id); setPlateIndex(undefined); setShowViewer(true); }}><Eye />{t("View")}</button></header><div className="profile-grid">{profile.printer && <div><span>{t("Printer")}</span><strong>{profile.printer}</strong></div>}{profile.printProfile && <div><span>{t("Profile")}</span><strong>{profile.printProfile}</strong></div>}{profile.nozzleDiameterMm != null && <div><span>{t("Nozzle")}</span><strong>{profile.nozzleDiameterMm} mm</strong></div>}{profile.layerHeightMm != null && <div><span>{t("Layer")}</span><strong>{profile.layerHeightMm} mm</strong></div>}</div>{profile.plates.length > 0 && <div className="plate-list">{profile.plates.map((plate) => <button key={plate.index} disabled={!detail.data} onClick={() => { setViewerAssetId(asset.id); setPlateIndex(plate.index); setShowViewer(true); }}><PlateThumbnail credential={credential} assetId={asset.id} plateIndex={plate.index} name={t("Plate {number}", { number: plate.index })} enabled={Boolean(detail.data)} /><span><strong>{t("Plate {number}", { number: plate.index })}</strong><small>{plate.objectCount} {t("objects")}{plate.printTimeSeconds ? ` · ${formatDuration(plate.printTimeSeconds)}` : ""}{plate.filamentGrams ? ` · ${Math.round(plate.filamentGrams)} g` : ""}</small>{plate.materialNames.length > 0 && <em>{plate.materialNames.join(" · ")}</em>}</span><ChevronRight /></button>)}</div>}{!detail.data && profile.plates.length > 0 && <p className="offline-plate-note"><CloudOff />{t("Plate-specific previews need a connection to Volum Desktop. The saved full model remains available.")}</p>}</article>; })}</div></DetailSection>}
      <DetailSection title={t("Geometry & inspection")} icon={<Gauge />}><div className="inspection-list">{project.assets.map((asset) => <article key={asset.id}><header><span>{asset.extension}</span><div><strong>{asset.filename}</strong><small>{t(asset.parseStatus)}{asset.missing ? ` · ${t("Missing")}` : ""}</small></div></header><div>{asset.metadata.dimensionsMm && <span><Ruler />{asset.metadata.dimensionsMm.map((value) => Math.round(value)).join(" × ")} mm</span>}{asset.metadata.triangleCount != null && <span><Layers3 />{formatNumber(asset.metadata.triangleCount)} {t("triangles")}</span>}{asset.metadata.objectCount != null && <span><Boxes />{asset.metadata.objectCount} {t("objects")}</span>}{asset.metadata.surfaceAreaMm2 != null && <span><Scale />{formatArea(asset.metadata.surfaceAreaMm2)}</span>}{asset.metadata.volumeMm3 != null && <span><Box />{formatVolume(asset.metadata.volumeMm3)}</span>}</div>{asset.metadata.materialNames?.length ? <small className="inspection-materials">{asset.metadata.materialNames.join(" · ")}</small> : null}{asset.metadata.warning && <p><CircleAlert />{t(asset.metadata.warning)}</p>}</article>)}</div></DetailSection>
      <DetailSection title={t("Print cost")} icon={<Scale />}><div className="cost-card">{project.estimate ? <><div className="cost-total"><span>{t(project.estimate.quantity > 1 ? "Batch estimate" : "Estimated cost")}</span><strong>{formatMoney(project.estimate.batchCostMinor ?? project.estimate.costPerPieceMinor, materials.data?.find((item) => item.id === project.estimate?.materialId)?.currency)}</strong></div><div className="cost-facts"><span>{formatNumber(project.estimate.plasticG)} {t("g each")}</span><span>{project.estimate.quantity} {t(project.estimate.quantity === 1 ? "piece" : "pieces")}</span><span>{materials.data?.find((item) => item.id === project.estimate?.materialId)?.name ?? t(project.estimate.source)}</span></div></> : <p className="prose prose--empty">{t("No saved material estimate yet.")}</p>}{canWrite && <button className="inline-edit" onClick={beginCost}><Pencil />{project.estimate ? t("Edit estimate") : t("Add estimate")}</button>}</div></DetailSection>
      {docs && (canWrite || docs.photos.length > 0) && <DetailSection title={t("Workshop photos")} icon={<FileImage />} count={docs.photos.length}><div className="documentation-section">{canWrite && <div className="photo-actions"><button onClick={() => choosePhoto(true)}><Camera />{t("Take photo")}</button><button onClick={() => choosePhoto(false)}><FileImage />{t("Photo library")}</button></div>}{docs.photos.length ? <div className="photo-grid photo-grid--managed">{docs.photos.map((photo, index) => <article key={photo.id}><button className="photo-grid__image" onClick={() => canWrite && setPhotoDraft({ photo, caption: photo.caption, isCover: photo.isCover })}><PhotoImage credential={credential} photo={photo} offlineBlob={offlinePhotoMap.get(photo.id)} />{photo.isCover && <span>{t("Cover")}</span>}</button><div><small>{photo.caption || photo.originalFilename}</small>{canWrite && <span className="photo-row-actions"><button disabled={index === 0 || Boolean(documentationBusy)} aria-label={t("Move photo up")} onClick={() => void movePhoto(photo, -1)}><ArrowUp /></button><button disabled={index === docs.photos.length - 1 || Boolean(documentationBusy)} aria-label={t("Move photo down")} onClick={() => void movePhoto(photo, 1)}><ArrowDown /></button><button aria-label={t("Edit photo")} onClick={() => setPhotoDraft({ photo, caption: photo.caption, isCover: photo.isCover })}><Pencil /></button><button aria-label={t("Delete photo")} onClick={() => void removePhoto(photo)}>{documentationBusy === `photo:${photo.id}` ? <LoaderCircle className="spin" /> : <Trash2 />}</button></span>}</div></article>)}</div> : <p className="prose prose--empty">{t("Capture the build, a finished print, or an assembly detail.")}</p>}</div></DetailSection>}
      {docs && (canWrite ? Boolean(fields.data?.length) : docs.customValues.length > 0) && <DetailSection title={t("Project details")} icon={<TagIcon />} count={docs.customValues.length}><div className={canWrite ? "custom-value-list" : "metadata-list"}>{canWrite ? fields.data?.map((field) => <CustomValueControl key={field.id} field={field} value={docs.customValues.find((item) => item.fieldId === field.id)?.value} busy={documentationBusy === `field:${field.id}`} onSave={(value) => void saveCustomValue(field.id, value)} />) : docs.customValues.map((item) => { const field = fieldMap.get(item.fieldId); return <div key={item.fieldId}><span>{field?.name ?? t("Custom field")}</span><strong>{String(item.value ?? "—")}{field?.unit ? ` ${field.unit}` : ""}</strong></div>; })}</div></DetailSection>}
      {docs && (canWrite || docs.bomItems.length > 0) && <DetailSection title={t("Bill of materials")} icon={<Package />} count={docs.bomItems.length}><div className="documentation-section"><div className="section-tools"><p>{t("Printed files and purchased parts for this build.")}</p>{canWrite && <button className="inline-edit" onClick={() => editBomItem()}><Plus />{t("Add item")}</button>}</div>{docs.bomItems.length ? <div className="bom-list">{docs.bomItems.map((item) => <article key={item.id}><span className={`bom-kind bom-kind--${item.kind}`}>{item.kind === "printed" ? <Box /> : <Package />}</span><div><strong>{item.name}</strong><small>{item.quantity} {item.unit}{item.notes ? ` · ${item.notes}` : ""}</small></div>{canWrite && <div className="row-actions"><button aria-label={t("Move {name} up", { name: item.name })} disabled={Boolean(documentationBusy)} onClick={() => void moveBomItem(item, -1)}><ArrowUp /></button><button aria-label={t("Move {name} down", { name: item.name })} disabled={Boolean(documentationBusy)} onClick={() => void moveBomItem(item, 1)}><ArrowDown /></button><button aria-label={t("Edit {name}", { name: item.name })} disabled={Boolean(documentationBusy)} onClick={() => editBomItem(item)}><Pencil /></button><button aria-label={t("Delete {name}", { name: item.name })} disabled={Boolean(documentationBusy)} onClick={() => void removeBomItem(item)}>{documentationBusy === `bom:${item.id}` ? <LoaderCircle className="spin" /> : <Trash2 />}</button></div>}</article>)}</div> : <p className="prose prose--empty">{t("No parts have been added yet.")}</p>}</div></DetailSection>}
      {docs && (canWrite || docs.assemblySteps.length > 0) && <DetailSection title={t("Assembly guide")} icon={<ListChecks />} count={docs.assemblySteps.length}><div className="documentation-section"><div className="section-tools"><p>{t("Ordered build instructions linked to parts and photos.")}</p><div className="section-tools__actions">{docs.assemblySteps.length > 0 && <button className="inline-edit" onClick={() => setWorkshopMode(true)}><Wrench />{t("Workshop mode")}</button>}{canWrite && <button className="inline-edit" onClick={() => editAssemblyStep()}><Plus />{t("Add step")}</button>}</div></div>{docs.assemblySteps.length ? <div className="step-list">{docs.assemblySteps.map((step, index) => <article className="assembly-step" key={step.id}><span>{index + 1}</span><div><strong>{step.title || t("Step {number}", { number: index + 1 })}</strong>{step.instructions && <p>{step.instructions}</p>}<small>{step.bomItemIds.length} {t("parts")} · {step.photoIds.length} {t("photos")}</small>{step.photoIds.length > 0 && <div className="step-photos">{step.photoIds.map((photoId) => { const photo = docs.photos.find((candidate) => candidate.id === photoId); return photo ? <PhotoImage key={photoId} credential={credential} photo={photo} offlineBlob={offlinePhotoMap.get(photo.id)} /> : null; })}</div>}</div>{canWrite && <div className="row-actions"><button aria-label={t("Move {name} up", { name: step.title })} disabled={Boolean(documentationBusy)} onClick={() => void moveStep(step, -1)}><ArrowUp /></button><button aria-label={t("Move {name} down", { name: step.title })} disabled={Boolean(documentationBusy)} onClick={() => void moveStep(step, 1)}><ArrowDown /></button><button aria-label={t("Edit {name}", { name: step.title })} disabled={Boolean(documentationBusy)} onClick={() => editAssemblyStep(step)}><Pencil /></button><button aria-label={t("Delete {name}", { name: step.title })} disabled={Boolean(documentationBusy)} onClick={() => void removeStep(step)}>{documentationBusy === `step:${step.id}` ? <LoaderCircle className="spin" /> : <Trash2 />}</button></div>}</article>)}</div> : <p className="prose prose--empty">{t("No assembly steps have been added yet.")}</p>}</div></DetailSection>}
      <DetailSection title={t("Files")} icon={<Boxes />} count={project.assets.length}><div className="file-list file-list--actions">{project.assets.map((asset) => <div key={asset.id} className={viewerAsset?.id === asset.id ? "is-viewer-file" : ""}><span>{asset.extension}</span><div><strong>{asset.filename}</strong><small>{formatBytes(asset.byteSize)} · {t(asset.role)}</small></div>{["stl", "3mf", "obj", "step", "stp"].includes(asset.extension.toLowerCase()) && !asset.missing && <button aria-label={t("View {name} in 3D", { name: asset.filename })} disabled={!detail.data && asset.id !== offlineViewerAssetId} onClick={() => { setViewerAssetId(asset.id); setPlateIndex(undefined); setShowViewer(true); }}><Eye /></button>}</div>)}</div>{detail.data && <button className="upload-file-button" disabled={uploading} onClick={() => void addFile()}>{uploading ? <LoaderCircle className="spin" /> : <UploadCloud />}{uploading && uploadProgress ? `${t("Uploading")} ${Math.round(uploadProgress.uploadedBytes / uploadProgress.totalBytes * 100)}%` : t("Add a 3D file")}</button>}{uploading && uploadProgress && <div className="upload-progress"><span style={{ width: `${uploadProgress.uploadedBytes / uploadProgress.totalBytes * 100}%` }} /></div>}{uploading && fileUploadId && <button className="cancel-upload-button" disabled={cancelingUpload === fileUploadId} onClick={() => void cancelActiveUpload(fileUploadId)}><X />{cancelingUpload === fileUploadId ? t("Canceling…") : t("Cancel upload")}</button>}{uploadError && <div className="error-banner" role="alert"><CircleAlert size={16} />{uploadError}</div>}</DetailSection>
      {related.data?.length ? <DetailSection title={t("Related projects")} icon={<Layers3 />} count={related.data.length}><div className="related-list">{related.data.map((item) => <button key={item.id} onClick={() => onOpen(item.id)}><span>{item.primaryExtension || "3D"}</span><div><strong>{item.displayName}</strong><small>{t(item.relationship)} · {item.relativeFolder}</small></div><ChevronRight /></button>)}</div></DetailSection> : null}
      {project.webSource && <DetailSection title={t("Source")} icon={<Wifi />}><div className="source-card"><strong>{project.webSource.title}</strong><span>{project.webSource.creator ? `${project.webSource.creator} · ` : ""}{project.webSource.provider}</span>{project.webSource.license && <small>{project.webSource.license}</small>}<button className="inline-edit" onClick={() => void openSource()}><ExternalLink />{t("Open source page")}</button></div></DetailSection>}
    </article>
    {editingName && <div className="sheet-scrim" role="presentation" onClick={() => !savingProject && setEditingName(false)}><section className="mobile-sheet" role="dialog" aria-modal="true" aria-labelledby="rename-project-title" onClick={(event) => event.stopPropagation()}><header><div><div className="eyebrow">{t("PROJECT")}</div><h2 id="rename-project-title">{t("Rename project")}</h2></div><button className="round-button" aria-label={t("Close")} disabled={savingProject} onClick={() => setEditingName(false)}><X /></button></header><label className="sheet-field"><span>{t("Name")}</span><input value={nameDraft} maxLength={160} autoFocus onChange={(event) => setNameDraft(event.target.value)} /></label>{editError && <div className="error-banner" role="alert"><CircleAlert size={16} />{editError}</div>}<button className="button button--primary button--large sheet-submit" disabled={!nameDraft.trim() || nameDraft.trim() === project.displayName || savingProject} onClick={() => void saveName()}>{savingProject ? <LoaderCircle className="spin" /> : <Check />}{savingProject ? t("Renaming…") : t("Save name")}</button></section></div>}
    {shareOpen && <div className="sheet-scrim" role="presentation" onClick={() => { if (!shareBusy) { setShareOpen(false); setPreparedShare(undefined); } }}><section className="mobile-sheet mobile-sheet--tall project-share-sheet" role="dialog" aria-modal="true" aria-labelledby="share-project-title" onClick={(event) => event.stopPropagation()}><header><div><div className="eyebrow">{t("PROJECT FILES")}</div><h2 id="share-project-title">{t("Share or save a file")}</h2></div><button className="round-button" aria-label={t("Close")} disabled={Boolean(shareBusy)} onClick={() => { setShareOpen(false); setPreparedShare(undefined); }}><X /></button></header><p>{t("Send the original file with AirDrop, Messages, email, or another app.")}</p>{shareMessage && <div className="success-banner" role="status"><Check />{shareMessage}</div>}{editError && <div className="error-banner" role="alert"><CircleAlert />{editError}</div>}<div className="share-file-list">{project.assets.filter((asset) => !asset.missing).sort((left, right) => Number(right.id === project.primaryAssetId) - Number(left.id === project.primaryAssetId)).map((asset) => <article key={asset.id}><span>{asset.extension}</span><div><strong>{asset.filename}</strong><small>{formatBytes(asset.byteSize)} · {t(asset.role)}</small></div><div className="share-file-actions"><button className={preparedShare?.assetId === asset.id ? "is-ready" : ""} disabled={Boolean(shareBusy)} onClick={() => void shareAsset(asset)}>{shareBusy === `share:${asset.id}` ? <LoaderCircle className="spin" /> : <Share2 />}<span>{preparedShare?.assetId === asset.id ? t("Share now") : t("Share")}</span></button><button disabled={Boolean(shareBusy)} onClick={() => void downloadAsset(asset)}>{shareBusy === `save:${asset.id}` ? <LoaderCircle className="spin" /> : <Save />}<span>{t("Save to Files")}</span></button></div></article>)}</div></section></div>}
    {projectQr && <div className="sheet-scrim" role="presentation" onClick={() => setProjectQr("")}><section className="mobile-sheet project-qr-sheet" role="dialog" aria-modal="true" aria-labelledby="project-qr-title" onClick={(event) => event.stopPropagation()}><header><div><div className="eyebrow">{t("PROJECT HANDOFF")}</div><h2 id="project-qr-title">{t("Scan to open")}</h2></div><button className="round-button" aria-label={t("Close")} onClick={() => setProjectQr("")}><X /></button></header><img src={projectQr} alt={t("QR code for {name}", { name: project.displayName })} /><strong>{project.displayName}</strong><p>{t("Scan from Library on another device paired with")} <strong>{credential.serverName}</strong>. {t("The code contains project IDs only—never your token or file paths.")}</p></section></div>}
    {editingTags && <div className="sheet-scrim" role="presentation" onClick={() => !savingProject && !creatingTag && setEditingTags(false)}><section className="mobile-sheet mobile-sheet--tall" role="dialog" aria-modal="true" aria-labelledby="edit-tags-title" onClick={(event) => event.stopPropagation()}><header><div><div className="eyebrow">{t("ORGANIZE")}</div><h2 id="edit-tags-title">{t("Project tags")}</h2></div><button className="round-button" aria-label={t("Close")} disabled={savingProject || creatingTag} onClick={() => setEditingTags(false)}><X /></button></header><div className="tag-picker">{tags.isLoading ? <LoaderCircle className="spin" /> : tags.data?.length ? tags.data.map((tag) => { const selected = tagDraft.includes(tag.id); return <div className="tag-picker__row" key={tag.id} style={{ "--tag-color": tag.color } as React.CSSProperties}><button className={selected ? "is-selected" : ""} onClick={() => setTagDraft((current) => selected ? current.filter((id) => id !== tag.id) : [...current, tag.id])}><i /><span>{tag.name}</span>{selected && <Check />}</button><button className="tag-picker__edit" aria-label={t("Manage {name}", { name: tag.name })} onClick={() => { setEditError(""); setTagEditor({ tag, name: tag.name, color: tag.color }); }}><Pencil /></button></div>; }) : <p className="prose prose--empty">{t("Create the first tag below.")}</p>}</div><div className="new-tag"><label className="sheet-field"><span>{t("New tag")}</span><div><input value={newTagName} maxLength={40} placeholder="Prototype" onChange={(event) => setNewTagName(event.target.value)} /><button disabled={!newTagName.trim() || creatingTag} onClick={() => void createTag()}>{creatingTag ? <LoaderCircle className="spin" /> : <Plus />}</button></div></label><div className="tag-color-picker">{COLLECTION_COLORS.map((color) => <button key={color} className={newTagColor === color ? "is-selected" : ""} aria-label={t("Use {color}", { color })} onClick={() => setNewTagColor(color)} style={{ "--swatch": color } as React.CSSProperties} />)}</div></div>{editError && <div className="error-banner" role="alert"><CircleAlert size={16} />{editError}</div>}<button className="button button--primary button--large" disabled={savingProject || creatingTag} onClick={() => void saveTags()}>{savingProject ? <LoaderCircle className="spin" /> : <Check />}{savingProject ? t("Saving…") : t("Save tags")}</button></section></div>}
    {tagEditor && <div className="sheet-scrim sheet-scrim--nested" role="presentation" onClick={() => !tagEditorBusy && setTagEditor(undefined)}><form className="mobile-sheet" role="dialog" aria-modal="true" aria-labelledby="manage-tag-title" onSubmit={(event) => { event.preventDefault(); void saveManagedTag(); }} onClick={(event) => event.stopPropagation()}><header><div><div className="eyebrow">{t("TAG")}</div><h2 id="manage-tag-title">{t("Manage tag")}</h2></div><button type="button" className="round-button" aria-label={t("Close")} disabled={tagEditorBusy} onClick={() => setTagEditor(undefined)}><X /></button></header><label className="sheet-field"><span>{t("Name")}</span><input autoFocus value={tagEditor.name} maxLength={40} onChange={(event) => setTagEditor({ ...tagEditor, name: event.target.value })} /></label><fieldset className="color-picker"><legend>{t("Color")}</legend><div>{COLLECTION_COLORS.map((color) => <button type="button" key={color} className={tagEditor.color === color ? "is-selected" : ""} aria-label={t("Use {color}", { color })} onClick={() => setTagEditor({ ...tagEditor, color })} style={{ "--swatch": color } as React.CSSProperties}>{tagEditor.color === color && <Check />}</button>)}</div></fieldset><p className="tag-usage">{t("Used by {count} {projects}.", { count: tagEditor.tag.modelCount, projects: t(tagEditor.tag.modelCount === 1 ? "project" : "projects") })}</p>{editError && <div className="error-banner" role="alert"><CircleAlert size={16} />{editError}</div>}<div className="collection-sheet-actions"><button type="button" className="button button--danger" disabled={tagEditorBusy} onClick={() => void deleteManagedTag()}><Trash2 />{t("Delete")}</button><button className="button button--primary button--large" disabled={tagEditorBusy || !tagEditor.name.trim()}>{tagEditorBusy ? <LoaderCircle className="spin" /> : <Check />}{t("Save tag")}</button></div></form></div>}
    {photoDraft && <div className="sheet-scrim" role="presentation" onClick={() => !documentationBusy && setPhotoDraft(undefined)}><form className="mobile-sheet documentation-sheet" role="dialog" aria-modal="true" aria-labelledby="photo-editor-title" onSubmit={(event) => { event.preventDefault(); void savePhoto(); }} onClick={(event) => event.stopPropagation()}><header><div><div className="eyebrow">{t("WORKSHOP PHOTO")}</div><h2 id="photo-editor-title">{photoDraft.file ? t("Add photo") : t("Photo details")}</h2></div><button type="button" className="round-button" aria-label={t("Close")} disabled={Boolean(documentationBusy)} onClick={() => setPhotoDraft(undefined)}><X /></button></header>{photoDraft.file && <div className="selected-photo"><FileImage /><div><strong>{photoDraft.file.name}</strong><small>{formatBytes(photoDraft.file.size)}</small></div></div>}<label className="sheet-field"><span>{t("Caption")}</span><textarea autoFocus value={photoDraft.caption} maxLength={500} placeholder={t("What does this photo show?")} onChange={(event) => setPhotoDraft({ ...photoDraft, caption: event.target.value })} /></label><label className="toggle-row"><input type="checkbox" checked={photoDraft.isCover} onChange={(event) => setPhotoDraft({ ...photoDraft, isCover: event.target.checked })} /><span><strong>{t("Use as project cover")}</strong><small>{t("Shown in the library and detail header.")}</small></span></label>{photoProgress && <div className="upload-status" role="status" aria-live="polite"><div><LoaderCircle className="spin" /><strong>{t("Sending photo")}</strong><span>{Math.round(photoProgress.uploadedBytes / photoProgress.totalBytes * 100)}%</span></div><div className="upload-progress"><span style={{ width: `${photoProgress.uploadedBytes / photoProgress.totalBytes * 100}%` }} /></div>{photoUploadId && <button type="button" className="cancel-upload-button" disabled={cancelingUpload === photoUploadId} onClick={() => void cancelActiveUpload(photoUploadId)}><X />{cancelingUpload === photoUploadId ? t("Canceling…") : t("Cancel upload")}</button>}</div>}<button className="button button--primary button--large sheet-submit" disabled={Boolean(documentationBusy)}>{documentationBusy ? <LoaderCircle className="spin" /> : <Check />}{photoDraft.file ? t("Add photo") : t("Save photo")}</button></form></div>}
    {costDraft && <div className="sheet-scrim" role="presentation" onClick={() => !savingProject && setCostDraft(undefined)}><form className="mobile-sheet documentation-sheet" role="dialog" aria-modal="true" aria-labelledby="cost-editor-title" onSubmit={(event) => { event.preventDefault(); void saveCost(); }} onClick={(event) => event.stopPropagation()}><header><div><div className="eyebrow">{t("PRINT COST")}</div><h2 id="cost-editor-title">{t("Material estimate")}</h2></div><button type="button" className="round-button" aria-label={t("Close")} disabled={savingProject} onClick={() => setCostDraft(undefined)}><X /></button></header><div className="sheet-field-grid"><label className="sheet-field sheet-field--wide"><span>{t("Project file")}</span><select value={costDraft.assetId} onChange={(event) => setCostDraft({ ...costDraft, assetId: event.target.value })}><option value="">{t("Whole project")}</option>{project.assets.map((asset) => <option key={asset.id} value={asset.id}>{asset.filename}</option>)}</select></label><label className="sheet-field sheet-field--wide"><span>{t("Material")}</span><select value={costDraft.materialId} onChange={(event) => setCostDraft({ ...costDraft, materialId: event.target.value })}><option value="">{t("No priced material")}</option>{materials.data?.map((material) => <option key={material.id} value={material.id}>{material.name} · {formatMoney(material.spoolPriceMinor, material.currency)} / {formatNumber(material.spoolWeightG)} g</option>)}</select></label><label className="sheet-field"><span>{t("Plastic per piece (g)")}</span><input type="number" min="0" max="1000000" step="any" value={costDraft.plasticG} onChange={(event) => setCostDraft({ ...costDraft, plasticG: event.target.value })} /></label><label className="sheet-field"><span>{t("Quantity")}</span><input type="number" min="1" max="100000" step="1" value={costDraft.quantity} onChange={(event) => setCostDraft({ ...costDraft, quantity: event.target.value })} /></label><label className="sheet-field sheet-field--wide"><span>{t("Weight source")}</span><select value={costDraft.source} onChange={(event) => setCostDraft({ ...costDraft, source: event.target.value as CostDraft["source"] })}><option value="manual">{t("Manual")}</option><option value="3mf">{t("3MF profile")}</option><option value="web">{t("Source page")}</option></select></label></div><button className="button button--primary button--large sheet-submit" disabled={savingProject || !Number.isFinite(Number(costDraft.plasticG)) || Number(costDraft.plasticG) < 0 || !Number.isInteger(Number(costDraft.quantity)) || Number(costDraft.quantity) < 1}>{savingProject ? <LoaderCircle className="spin" /> : <Check />}{t("Save estimate")}</button></form></div>}
    {bomDraft && <div className="sheet-scrim" role="presentation" onClick={() => !documentationBusy && setBomDraft(undefined)}><form className="mobile-sheet mobile-sheet--tall documentation-sheet" role="dialog" aria-modal="true" aria-labelledby="bom-item-title" onSubmit={(event) => { event.preventDefault(); void saveBomItem(); }} onClick={(event) => event.stopPropagation()}><header><div><div className="eyebrow">{t("BILL OF MATERIALS")}</div><h2 id="bom-item-title">{bomDraft.id ? t("Edit item") : t("Add item")}</h2></div><button type="button" className="round-button" aria-label={t("Close")} disabled={Boolean(documentationBusy)} onClick={() => setBomDraft(undefined)}><X /></button></header><div className="sheet-field-grid"><label className="sheet-field"><span>{t("Part type")}</span><select value={bomDraft.kind} onChange={(event) => setBomDraft({ ...bomDraft, kind: event.target.value })}><option value="purchased">{t("Purchased")}</option><option value="printed">{t("Printed")}</option></select></label><label className="sheet-field sheet-field--wide"><span>{t("Name")}</span><input autoFocus value={bomDraft.name} maxLength={160} onChange={(event) => setBomDraft({ ...bomDraft, name: event.target.value })} /></label><label className="sheet-field"><span>{t("Quantity")}</span><input type="number" min="0.0001" step="any" value={bomDraft.quantity} onChange={(event) => setBomDraft({ ...bomDraft, quantity: Number(event.target.value) })} /></label><label className="sheet-field"><span>{t("Unit")}</span><input value={bomDraft.unit} maxLength={24} onChange={(event) => setBomDraft({ ...bomDraft, unit: event.target.value })} /></label><label className="sheet-field sheet-field--wide"><span>{t("Linked project file")}</span><select value={bomDraft.linkedAssetId ?? ""} onChange={(event) => setBomDraft({ ...bomDraft, linkedAssetId: event.target.value || undefined })}><option value="">{t("No linked file")}</option>{project.assets.map((asset) => <option key={asset.id} value={asset.id}>{asset.filename}</option>)}</select></label><label className="sheet-field sheet-field--wide"><span>{t("Purchase link")}</span><input type="url" value={bomDraft.url ?? ""} placeholder="https://…" onChange={(event) => setBomDraft({ ...bomDraft, url: event.target.value || undefined })} /></label><label className="sheet-field sheet-field--wide"><span>{t("Notes")}</span><textarea value={bomDraft.notes} onChange={(event) => setBomDraft({ ...bomDraft, notes: event.target.value })} /></label></div>{editError && <div className="error-banner" role="alert"><CircleAlert size={16} />{editError}</div>}<button className="button button--primary button--large sheet-submit" disabled={documentationBusy === "bom" || !bomDraft.name.trim() || !bomDraft.unit.trim() || !Number.isFinite(bomDraft.quantity) || bomDraft.quantity <= 0}>{documentationBusy === "bom" ? <LoaderCircle className="spin" /> : <Check />}{documentationBusy === "bom" ? t("Saving…") : bomDraft.id ? t("Save item") : t("Add item")}</button></form></div>}
    {stepDraft && <div className="sheet-scrim" role="presentation" onClick={() => !documentationBusy && setStepDraft(undefined)}><form className="mobile-sheet mobile-sheet--tall documentation-sheet" role="dialog" aria-modal="true" aria-labelledby="assembly-step-title" onSubmit={(event) => { event.preventDefault(); void saveStep(); }} onClick={(event) => event.stopPropagation()}><header><div><div className="eyebrow">{t("ASSEMBLY GUIDE")}</div><h2 id="assembly-step-title">{stepDraft.id ? t("Edit step") : t("Add step")}</h2></div><button type="button" className="round-button" aria-label={t("Close")} disabled={Boolean(documentationBusy)} onClick={() => setStepDraft(undefined)}><X /></button></header><div className="sheet-field-grid"><label className="sheet-field sheet-field--wide"><span>{t("Step title")}</span><input autoFocus value={stepDraft.title} maxLength={160} onChange={(event) => setStepDraft({ ...stepDraft, title: event.target.value })} /></label><label className="sheet-field sheet-field--wide"><span>{t("Instructions")}</span><textarea value={stepDraft.instructions} onChange={(event) => setStepDraft({ ...stepDraft, instructions: event.target.value })} /></label></div>{docs?.bomItems.length ? <fieldset className="link-picker"><legend>{t("Parts used")}</legend>{docs.bomItems.map((item) => { const selected = stepDraft.bomItemIds.includes(item.id); return <button type="button" key={item.id} className={selected ? "is-selected" : ""} onClick={() => setStepDraft({ ...stepDraft, bomItemIds: selected ? stepDraft.bomItemIds.filter((candidate) => candidate !== item.id) : [...stepDraft.bomItemIds, item.id] })}><span>{selected && <Check />}</span>{item.name}</button>; })}</fieldset> : null}{docs?.photos.length ? <fieldset className="link-picker"><legend>{t("Linked photos")}</legend>{docs.photos.map((photo) => { const selected = stepDraft.photoIds.includes(photo.id); return <button type="button" key={photo.id} className={selected ? "is-selected" : ""} onClick={() => setStepDraft({ ...stepDraft, photoIds: selected ? stepDraft.photoIds.filter((candidate) => candidate !== photo.id) : [...stepDraft.photoIds, photo.id] })}><span>{selected && <Check />}</span>{photo.caption || photo.originalFilename}</button>; })}</fieldset> : null}{editError && <div className="error-banner" role="alert"><CircleAlert size={16} />{editError}</div>}<button className="button button--primary button--large sheet-submit" disabled={documentationBusy === "step" || !stepDraft.title.trim()}>{documentationBusy === "step" ? <LoaderCircle className="spin" /> : <Check />}{documentationBusy === "step" ? t("Saving…") : stepDraft.id ? t("Save step") : t("Add step")}</button></form></div>}
  </main>;
}

function WorkshopScreen({ credential, project, documentation, offlinePhotos, onBack }: { credential: Credential; project: ProjectDetail; documentation: ProjectDocumentation; offlinePhotos: Map<string, Blob>; onBack: () => void }) {
  const [index, setIndex] = useState(0);
  const step = documentation.assemblySteps[index];
  useEffect(() => {
    let lock: WakeLockSentinel | undefined;
    void navigator.wakeLock?.request("screen").then((value) => { lock = value; }).catch(() => undefined);
    return () => { void lock?.release(); };
  }, []);
  if (!step) return <main className="workshop-screen"><header><button className="round-button" aria-label={t("Leave workshop mode")} onClick={onBack}><ArrowLeft /></button><div><span>{t("WORKSHOP MODE")}</span><strong>{project.displayName}</strong></div></header><div className="center-state"><ListChecks /><strong>{t("No assembly steps")}</strong><p>{t("Add instructions from the project detail view first.")}</p></div></main>;
  const parts = step.bomItemIds.map((id) => documentation.bomItems.find((item) => item.id === id)).filter(Boolean) as ProjectBomItem[];
  const photos = step.photoIds.map((id) => documentation.photos.find((photo) => photo.id === id)).filter(Boolean) as ProjectPhoto[];
  return <main className="workshop-screen">
    <header><button className="round-button" aria-label={t("Leave workshop mode")} onClick={onBack}><ArrowLeft /></button><div><span>{t("WORKSHOP MODE · {current} OF {total}", { current: index + 1, total: documentation.assemblySteps.length })}</span><strong>{project.displayName}</strong></div><Wrench /></header>
    <div className="workshop-progress"><span style={{ width: `${(index + 1) / documentation.assemblySteps.length * 100}%` }} /></div>
    <article><div className="workshop-step-number">{index + 1}</div><h1>{step.title || t("Step {number}", { number: index + 1 })}</h1>{step.instructions && <p>{step.instructions}</p>}{photos.length > 0 && <div className="workshop-photos">{photos.map((photo) => <PhotoImage key={photo.id} credential={credential} photo={photo} full offlineBlob={offlinePhotos.get(photo.id)} />)}</div>}{parts.length > 0 && <section><span>{t("PARTS FOR THIS STEP")}</span>{parts.map((part) => <div key={part.id}><strong>{part.name}</strong><small>{part.quantity} {part.unit}</small></div>)}</section>}</article>
    <footer><button disabled={index === 0} onClick={() => setIndex((value) => Math.max(0, value - 1))}><ArrowLeft />{t("Previous")}</button><button className="is-primary" disabled={index === documentation.assemblySteps.length - 1} onClick={() => setIndex((value) => Math.min(documentation.assemblySteps.length - 1, value + 1))}>{t("Next")}<ChevronRight /></button></footer>
  </main>;
}

function DetailSection({ title, icon, count, children }: { title: string; icon: React.ReactNode; count?: number; children: React.ReactNode }) {
  return <section className="detail-section"><header><span className="detail-section__icon">{icon}</span><div><strong>{title}</strong>{typeof count === "number" && <span className="detail-section__count">{count}</span>}</div></header><div className="detail-section__body">{children}</div></section>;
}
