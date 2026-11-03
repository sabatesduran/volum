import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Boxes, Check, ChevronRight, CircleAlert, CloudCheck, CloudDownload, LoaderCircle, Pencil, Plus, Trash2, X } from "lucide-react";
import { useEffect, useState } from "react";
import { ErrorState } from "../../components/ErrorState";
import { PullToRefresh } from "../../components/PullToRefresh";
import { COLLECTION_COLORS } from "../../lib/colors";
import { formatBytes, formatDateTime } from "../../lib/format";
import { localizedError, t } from "../../lib/i18n";
import { listOfflineManifests, listOfflineScopes, offlineStorageEstimate, removeOfflineProject, removeOfflineScope, syncOfflineScope, type OfflineScopeInput } from "../../lib/offline";
import { mutateJson, requestJson } from "../../lib/transport";
import type { Collection, Credential, OfflineManifest, OfflineScope, ServerStatus } from "../../types";

export function CollectionsScreen({ credential, canWrite, verified, onSelect, onOpen }: { credential: Credential; canWrite: boolean; verified: boolean; onSelect: (id: string) => void; onOpen: (id: string) => void }) {
  const queryClient = useQueryClient();
  const [editing, setEditing] = useState<Collection | "new">();
  const [name, setName] = useState("");
  const [color, setColor] = useState(COLLECTION_COLORS[0]);
  const [saving, setSaving] = useState(false);
  const [syncingScope, setSyncingScope] = useState("");
  const [error, setError] = useState("");
  const query = useQuery({ queryKey: ["collections"], queryFn: () => requestJson<Collection[]>(credential, "/api/v1/collections"), enabled: verified });
  const offline = useQuery({ queryKey: ["offline-manifests", credential.serverId], queryFn: () => listOfflineManifests(credential.serverId), staleTime: Infinity });
  const scopes = useQuery({ queryKey: ["offline-scopes", credential.serverId], queryFn: () => listOfflineScopes(credential.serverId), staleTime: Infinity });
  const storage = useQuery({ queryKey: ["offline-storage"], queryFn: offlineStorageEstimate });
  const libraryRevision = queryClient.getQueryData<ServerStatus>(["status", credential.baseUrl])?.libraryRevision;
  useEffect(() => {
    const refresh = () => { void scopes.refetch(); void offline.refetch(); };
    window.addEventListener("volum-offline-change", refresh);
    return () => window.removeEventListener("volum-offline-change", refresh);
  }, [offline, scopes]);
  const collections = query.data ?? [];
  const beginEdit = (collection?: Collection) => { setError(""); setEditing(collection ?? "new"); setName(collection?.name ?? ""); setColor(collection?.color ?? COLLECTION_COLORS[0]); };
  const save = async () => {
    if (!name.trim() || !editing) return;
    const activeEditor = editing;
    const previous = queryClient.getQueryData<Collection[]>(["collections"]);
    const temporaryId = `optimistic-${crypto.randomUUID()}`;
    const optimistic: Collection = activeEditor === "new" ? {
      id: temporaryId,
      name: name.trim(),
      symbol: "boxes",
      color,
      modelCount: 0,
      smart: false,
      createdAt: new Date().toISOString(),
    } : { ...activeEditor, name: name.trim(), color };
    setSaving(true); setError("");
    setEditing(undefined);
    queryClient.setQueryData<Collection[]>(["collections"], (current = []) => activeEditor === "new"
      ? [...current, optimistic]
      : current.map((collection) => collection.id === activeEditor.id ? optimistic : collection));
    try {
      const saved = activeEditor === "new"
        ? await mutateJson<Collection>(credential, "/api/v1/collections", "POST", { name: name.trim(), color })
        : await mutateJson<Collection>(credential, `/api/v1/collections/${encodeURIComponent(activeEditor.id)}`, "PATCH", { name: name.trim(), color });
      queryClient.setQueryData<Collection[]>(["collections"], (current = []) => current.map((collection) => collection.id === optimistic.id ? saved : collection));
      await queryClient.invalidateQueries({ queryKey: ["collections"] });
    } catch (reason) {
      queryClient.setQueryData(["collections"], previous);
      setEditing(activeEditor);
      setError(localizedError(reason));
    }
    finally { setSaving(false); }
  };
  const removeCollection = async () => {
    if (!editing || editing === "new" || !window.confirm(t("Delete “{name}”? Projects and files will not be deleted.", { name: editing.name }))) return;
    const activeEditor = editing;
    const previous = queryClient.getQueryData<Collection[]>(["collections"]);
    setSaving(true); setError("");
    setEditing(undefined);
    queryClient.setQueryData<Collection[]>(["collections"], (current = []) => current.filter((collection) => collection.id !== activeEditor.id));
    try {
      await mutateJson<void>(credential, `/api/v1/collections/${encodeURIComponent(activeEditor.id)}`, "DELETE");
      await queryClient.invalidateQueries({ queryKey: ["collections"] });
    } catch (reason) {
      queryClient.setQueryData(["collections"], previous);
      setEditing(activeEditor);
      setError(localizedError(reason));
    }
    finally { setSaving(false); }
  };
  const removeOffline = async (manifest: OfflineManifest) => {
    await removeOfflineProject(credential.serverId, manifest.projectId);
    await Promise.all([offline.refetch(), storage.refetch()]);
  };
  const syncScope = async (scope: OfflineScope) => {
    setSyncingScope(scope.key);
    try {
      await syncOfflineScope(credential, scope as OfflineScopeInput, libraryRevision);
    } catch {
      await scopes.refetch();
    } finally {
      setSyncingScope("");
      await Promise.all([scopes.refetch(), offline.refetch()]);
    }
  };
  const deleteScope = async (scope: OfflineScope) => {
    await removeOfflineScope(scope.key);
    await scopes.refetch();
  };
  return <main className="library-screen"><PullToRefresh onRefresh={() => Promise.all([...(verified ? [query.refetch()] : []), offline.refetch(), scopes.refetch(), storage.refetch()])} /><header className="mobile-header"><div><h1>{t("Collections")}</h1><p>{t("Projects, smart views, and offline packs")}</p></div>{canWrite && <div className="mobile-header__actions"><button className="round-button" aria-label={t("Create collection")} onClick={() => beginEdit()}><Plus /></button></div>}</header>
    {verified && <><section className="organize-heading"><div><Boxes /><span><strong>{t("Your collections")}</strong><small>{t("Manual and smart groups from Desktop")}</small></span></div></section>
     {query.isLoading ? (
       <div className="center-state"><LoaderCircle className="spin" /></div>
     ) : query.isError ? (
       <ErrorState error={query.error} retrying={query.isFetching} onRetry={() => void query.refetch()} />
     ) : collections.length ? (
       <div className="collection-list">
         {collections.map((collection) => (
           <article key={collection.id} style={{ "--collection-color": collection.color } as React.CSSProperties}>
             <button onClick={() => onSelect(collection.id)}>
               <span><Boxes /></span>
               <div>
                 <strong>{collection.name}</strong>
                 <small>{collection.smart ? t("Smart collection") : t("Collection")} · {collection.modelCount} {t("projects")}</small>
               </div>
               <ChevronRight />
             </button>
             {canWrite && (
               <button
                 className="collection-list__edit"
                 aria-label={t("Edit {name}", { name: collection.name })}
                 onClick={() => beginEdit(collection)}
               >
                 <Pencil />
               </button>
             )}
           </article>
         ))}
       </div>
     ) : (
       <section className="collections-empty">
         <div className="collections-empty__art" aria-hidden="true"><span /><span /><span><Boxes /></span></div>
         <div className="eyebrow">{t("A PLACE FOR EVERY PROJECT")}</div>
         <h2>{t("No collections yet")}</h2>
         <p>{t("Collections group builds, parts, and ideas without moving a file.")}</p>
         {canWrite && (
           <button className="button button--primary" onClick={() => beginEdit()}>
             <Plus />{t("Create a collection")}
           </button>
         )}
       </section>
     )}
    </>}
    <section className="organize-heading organize-heading--offline"><div><CloudCheck /><span><strong>{t("Available offline")}</strong><small>{offline.data?.length ?? 0} {t("projects")} · {formatBytes(offline.data?.reduce((total, item) => total + item.byteSize, 0) ?? 0)}{storage.data?.quota ? ` · ${Math.round((storage.data.usage / storage.data.quota) * 100)}% ${t("device storage used")}` : ""}</small></span></div></section>
    {offline.data?.length ? <div className="offline-list">{offline.data.map((manifest) => <article key={manifest.key}><button onClick={() => onOpen(manifest.projectId)}><CloudCheck /><span><strong>{manifest.displayName}</strong><small>{formatBytes(manifest.byteSize)} · {t("Synced")} {formatDateTime(manifest.syncedAt)}</small></span><ChevronRight /></button><button aria-label={t("Remove {name} from offline storage", { name: manifest.displayName })} onClick={() => void removeOffline(manifest)}><Trash2 /></button></article>)}</div> : <div className="offline-empty"><CloudDownload /><p>{t("Open a project, folder, or collection and choose")} <strong>{t("Available offline")}</strong>.</p></div>}
    {scopes.data?.length ? <section className="offline-scope-list"><h2>{t("Offline views")}</h2>{scopes.data.map((scope) => {
      const relative = new Intl.RelativeTimeFormat(navigator.language, { numeric: "auto" });
      const elapsed = Math.max(0, Date.now() - new Date(scope.syncedAt).getTime());
      const days = Math.floor(elapsed / 86_400_000);
      const hours = Math.floor(elapsed / 3_600_000);
      const minutes = Math.floor(elapsed / 60_000);
      const lastSynced = days ? relative.format(-days, "day") : hours ? relative.format(-hours, "hour") : relative.format(-minutes, "minute");
      return <article key={scope.key}><div><strong>{scope.label}</strong><small>{scope.projectIds.length} {t("projects")} · {t("Last synced {time}", { time: lastSynced })}</small>{scope.libraryRevision && libraryRevision && scope.libraryRevision !== libraryRevision && <small className="offline-scope-outdated">{t("Out of date")}</small>}{scope.error && <small className="offline-scope-error">{scope.error}</small>}</div><button disabled={!verified || !navigator.onLine || syncingScope === scope.key} onClick={() => void syncScope(scope)}>{syncingScope === scope.key ? <LoaderCircle className="spin" /> : t("Sync now")}</button><button aria-label={t("Remove offline view {name}", { name: scope.label })} onClick={() => void deleteScope(scope)}>{t("Remove")}</button></article>;
    })}</section> : null}
    {editing && <div className="sheet-scrim" role="presentation" onClick={() => !saving && setEditing(undefined)}><section className="mobile-sheet" role="dialog" aria-modal="true" aria-labelledby="collection-title" onClick={(event) => event.stopPropagation()}><header><div><div className="eyebrow">{t("COLLECTION")}</div><h2 id="collection-title">{editing === "new" ? t("Create collection") : t("Edit collection")}</h2></div><button className="round-button" aria-label={t("Close")} disabled={saving} onClick={() => setEditing(undefined)}><X /></button></header><label className="sheet-field"><span>{t("Name")}</span><input value={name} maxLength={80} autoFocus onChange={(event) => setName(event.target.value)} /></label><fieldset className="color-picker"><legend>{t("Color")}</legend><div>{COLLECTION_COLORS.map((value) => <button key={value} className={color === value ? "is-selected" : ""} aria-label={t("Use {color}", { color: value })} onClick={() => setColor(value)} style={{ "--swatch": value } as React.CSSProperties}>{color === value && <Check />}</button>)}</div></fieldset>{error && <div className="error-banner" role="alert"><CircleAlert size={16} />{error}</div>}<div className="collection-sheet-actions">{editing !== "new" && <button className="button button--danger" disabled={saving} onClick={() => void removeCollection()}><Trash2 />{t("Delete")}</button>}<button className="button button--primary button--large" disabled={!name.trim() || saving} onClick={() => void save()}>{saving ? <LoaderCircle className="spin" /> : <Check />}{editing === "new" ? t("Create") : t("Save")}</button></div></section></div>}
  </main>;
}
