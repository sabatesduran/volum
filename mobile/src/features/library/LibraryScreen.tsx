import { useInfiniteQuery, useQuery, useQueryClient } from "@tanstack/react-query";
import { cancel, Format, requestPermissions, scan } from "@tauri-apps/plugin-barcode-scanner";
import {
  Check, ChevronRight, CircleAlert, Clock3, CloudCheck, CloudDownload, Folder, Heart,
  LibraryBig, LoaderCircle, QrCode, Search, SlidersHorizontal, X, Boxes,
} from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { ErrorState } from "../../components/ErrorState";
import { PullToRefresh } from "../../components/PullToRefresh";
import { QrScannerOverlay } from "../../components/QrScannerOverlay";
import { notifyCompletion } from "../../lib/notifications";
import {
  downloadOfflineProject, listOfflineManifests, listOfflineProjectSnapshots,
} from "../../lib/offline";
import { parseProjectQrPayload } from "../../lib/projectQr";
import { CompanionError, requestJson } from "../../lib/transport";
import { localizedError, t } from "../../lib/i18n";
import type {
  Collection, CompanionPage, Credential, FolderSummary, ProjectDetail, ProjectSummary, SavedSearch, Tag,
} from "../../types";
import { ProjectImage } from "../project/ProjectMedia";

export type Tab = "library" | "favorites" | "upload" | "collections" | "settings";

function ProjectCard({ credential, project, offline, onOpen }: { credential: Credential; project: ProjectSummary; offline?: boolean; onOpen: () => void }) {
  return <button id={`project-${project.id}`} className="project-card" onClick={onOpen}>
    <ProjectImage credential={credential} projectId={project.id} revision={`${project.modifiedAt}:${project.coverPhotoId ?? "asset"}`} />
    <div className="project-card__copy"><strong>{project.displayName}</strong><span>{project.folderName} · {project.assetCount} {t(project.assetCount === 1 ? "file" : "files")}</span></div>
    <span className="project-card__format">{project.primaryExtension || "3D"}</span>
    {project.favorite && <Heart className="project-card__favorite" size={15} fill="currentColor" />}
    {offline && <CloudCheck className="project-card__offline" size={15} />}
  </button>;
}

export function LibraryScreen({ credential, tab, selectedCollection, onCollection, onOpen }: { credential: Credential; tab: Tab; selectedCollection?: string; onCollection: (id?: string) => void; onOpen: (id: string) => void }) {
  const queryClient = useQueryClient();
  const [searchValue, setSearchValue] = useState("");
  const [search, setSearch] = useState("");
  const [filtersOpen, setFiltersOpen] = useState(false);
  const [folderId, setFolderId] = useState("");
  const [savedSearchId, setSavedSearchId] = useState("");
  const [recentOnly, setRecentOnly] = useState(false);
  const [format, setFormat] = useState("");
  const [availability, setAvailability] = useState("");
  const [tagId, setTagId] = useState("");
  const [sort, setSort] = useState<"modified" | "added" | "opened" | "name">("modified");
  const [packBusy, setPackBusy] = useState(false);
  const [packProgress, setPackProgress] = useState({ completed: 0, total: 0 });
  const [packError, setPackError] = useState("");
  const [projectScannerOpen, setProjectScannerOpen] = useState(false);
  const [projectScanBusy, setProjectScanBusy] = useState(false);
  const [projectScanError, setProjectScanError] = useState("");
  const projectScanCanceled = useRef(false);
  const collections = useQuery({ queryKey: ["collections"], queryFn: () => requestJson<Collection[]>(credential, "/api/v1/collections") });
  const folders = useQuery({ queryKey: ["folders"], queryFn: () => requestJson<FolderSummary[]>(credential, "/api/v1/folders") });
  const savedSearches = useQuery({ queryKey: ["saved-searches"], queryFn: () => requestJson<SavedSearch[]>(credential, "/api/v1/saved-searches") });
  const tags = useQuery({ queryKey: ["tags"], queryFn: () => requestJson<Tag[]>(credential, "/api/v1/tags") });
  const offline = useQuery({ queryKey: ["offline-manifests", credential.serverId], queryFn: () => listOfflineManifests(credential.serverId), staleTime: Infinity });
  const offlineProjects = useQuery({ queryKey: ["offline-library", credential.serverId], queryFn: () => listOfflineProjectSnapshots(credential.serverId), staleTime: Infinity });
  useEffect(() => {
    const refresh = () => { void offline.refetch(); void offlineProjects.refetch(); };
    window.addEventListener("volum-offline-change", refresh);
    return () => window.removeEventListener("volum-offline-change", refresh);
  }, [offline, offlineProjects]);
  const savedQuery = savedSearches.data?.find((item) => item.id === savedSearchId)?.query;
  const buildParams = (cursor = "") => {
    const params = new URLSearchParams({ limit: "48", sort: savedQuery?.sort ?? sort });
    const effectiveSearch = search || savedQuery?.search;
    if (tab === "favorites" || savedQuery?.favorite) params.set("favorite", "true");
    if (tab === "library" && (recentOnly || savedQuery?.recent)) params.set("recent", "true");
    if (selectedCollection || savedQuery?.collectionId) params.set("collectionId", selectedCollection || savedQuery!.collectionId!);
    if (folderId || savedQuery?.folderId) params.set("folderId", folderId || savedQuery!.folderId!);
    if (effectiveSearch) params.set("search", effectiveSearch);
    if (format || savedQuery?.format) params.set("format", format || savedQuery!.format!);
    if (availability || savedQuery?.availability) params.set("availability", availability || savedQuery!.availability!);
    if (tagId || savedQuery?.tagId) params.set("tagId", tagId || savedQuery!.tagId!);
    if (savedQuery?.dateField) params.set("dateField", savedQuery.dateField);
    if (savedQuery?.dateFrom) params.set("dateFrom", savedQuery.dateFrom);
    if (savedQuery?.dateTo) params.set("dateTo", savedQuery.dateTo);
    if (cursor) params.set("cursor", cursor);
    return params;
  };
  const projects = useInfiniteQuery({
    queryKey: ["projects", tab, selectedCollection, folderId, savedSearchId, recentOnly, search, format, availability, tagId, sort],
    initialPageParam: "",
    queryFn: ({ pageParam }) => requestJson<CompanionPage<ProjectSummary>>(credential, `/api/v1/projects?${buildParams(pageParam)}`),
    getNextPageParam: (last) => last.nextCursor,
  });
  const items = projects.data?.pages.flatMap((page) => page.items) ?? [];
  const cachedItems = (offlineProjects.data ?? [])
    .map((snapshot) => snapshot.detail)
    .filter((project) => !(tab === "favorites" || savedQuery?.favorite) || project.favorite)
    .filter((project) => !(recentOnly || savedQuery?.recent) || Boolean(project.lastOpenedAt))
    .filter((project) => !(selectedCollection || savedQuery?.collectionId) || project.collectionIds.includes(selectedCollection || savedQuery!.collectionId!))
    .filter((project) => !(folderId || savedQuery?.folderId) || project.folderId === (folderId || savedQuery!.folderId))
    .filter((project) => !(format || savedQuery?.format) || project.primaryExtension.toLowerCase() === (format || savedQuery!.format))
    .filter((project) => {
      const selectedAvailability = availability || savedQuery?.availability;
      return !selectedAvailability || (selectedAvailability === "available" ? !project.missing : project.missing);
    })
    .filter((project) => !(tagId || savedQuery?.tagId) || project.tags.some((tag) => tag.id === (tagId || savedQuery!.tagId)))
    .filter((project) => {
      const needle = (search || savedQuery?.search || "").trim().toLocaleLowerCase();
      return !needle || `${project.displayName} ${project.folderName} ${project.notes}`.toLocaleLowerCase().includes(needle);
    })
    .sort((left, right) => {
      if ((savedQuery?.sort ?? sort) === "name") return left.displayName.localeCompare(right.displayName);
      if ((savedQuery?.sort ?? sort) === "added") return right.addedAt.localeCompare(left.addedAt);
      if ((savedQuery?.sort ?? sort) === "opened") return (right.lastOpenedAt ?? "").localeCompare(left.lastOpenedAt ?? "");
      return right.modifiedAt.localeCompare(left.modifiedAt);
    });
  const usingOfflineFallback = projects.isError && cachedItems.length > 0;
  const visibleItems = usingOfflineFallback ? cachedItems : items;
  const total = usingOfflineFallback ? cachedItems.length : projects.data?.pages[0]?.total;
  const activeFolder = folders.data?.find((item) => item.id === folderId);
  const activeSaved = savedSearches.data?.find((item) => item.id === savedSearchId);
  const title = tab === "favorites" ? t("Favorites") : recentOnly ? t("Recent") : activeSaved?.name ?? activeFolder?.name ?? (selectedCollection ? collections.data?.find((item) => item.id === selectedCollection)?.name ?? t("Collection") : t("Library"));
  const filterCount = [folderId, savedSearchId, recentOnly ? "recent" : "", format, availability, tagId, sort !== "modified" ? sort : ""].filter(Boolean).length;
  const offlineIds = new Set(offline.data?.map((manifest) => manifest.projectId));
  const clearScope = () => { setFolderId(""); setSavedSearchId(""); setRecentOnly(false); onCollection(undefined); };
  const saveScopeOffline = async () => {
    setPackBusy(true); setPackProgress({ completed: 0, total: 0 }); setPackError("");
    try {
      const all: ProjectSummary[] = [];
      let cursor = "";
      do {
        const params = buildParams(cursor); params.set("limit", "100");
        const page = await requestJson<CompanionPage<ProjectSummary>>(credential, `/api/v1/projects?${params}`);
        all.push(...page.items); cursor = page.nextCursor ?? "";
      } while (cursor);
      setPackProgress({ completed: 0, total: all.length });
      const failures: string[] = [];
      for (let index = 0; index < all.length; index += 1) {
        try {
          await downloadOfflineProject(credential, all[index].id);
        } catch (reason) {
          const message = localizedError(reason);
          failures.push(`${all[index].displayName}: ${message}`);
        }
        setPackProgress({ completed: index + 1, total: all.length });
      }
      await offline.refetch();
      if (failures.length) {
        const preview = failures.slice(0, 3).join(" · ");
        setPackError(t("{count} {projects} could not be saved. {details}", {
          count: failures.length,
          projects: t(failures.length === 1 ? "project" : "projects"),
          details: `${preview}${failures.length > 3 ? ` · ${t("{count} more", { count: failures.length - 3 })}` : ""}`,
        }));
      }
      await notifyCompletion(
        t(failures.length ? "Offline download finished with errors" : "Offline download complete"),
        failures.length
          ? t("{saved} of {total} projects were saved.", { saved: all.length - failures.length, total: all.length })
          : t("{count} projects are ready offline.", { count: all.length }),
      );
    } catch (reason) {
      setPackError(localizedError(reason));
    } finally { setPackBusy(false); }
  };
  const scanProject = async () => {
    projectScanCanceled.current = false;
    setProjectScanBusy(true); setProjectScanError("");
    try {
      const permission = await requestPermissions();
      if (permission !== "granted") throw new Error(t("Camera access is needed to scan a project code."));
      if (projectScanCanceled.current) return;
      const result = await scan({ formats: [Format.QRCode], cameraDirection: "back", windowed: true });
      if (projectScanCanceled.current) return;
      const payload = parseProjectQrPayload(result.content);
      if (payload.serverId !== credential.serverId) throw new Error(t("This project belongs to a different Volum Desktop."));
      const project = await requestJson<ProjectDetail>(credential, `/api/v1/projects/${encodeURIComponent(payload.projectId)}`);
      if (projectScanCanceled.current) return;
      queryClient.setQueryData(["project", payload.projectId], project);
      setProjectScannerOpen(false);
      onOpen(payload.projectId);
    } catch (reason) {
      const message = reason instanceof Error ? reason.message : String(reason);
      if (!projectScanCanceled.current && !/cancel/i.test(message)) {
        setProjectScanError(reason instanceof CompanionError && reason.code === "not_found" ? t("This project is no longer available.") : localizedError(reason));
      }
    }
    finally { setProjectScanBusy(false); }
  };
  const closeProjectScanner = async () => {
    projectScanCanceled.current = true;
    setProjectScannerOpen(false);
    if (projectScanBusy) await cancel().catch(() => undefined);
    setProjectScanBusy(false);
  };
  return <main className="library-screen">
    <PullToRefresh onRefresh={() => Promise.all([projects.refetch(), collections.refetch(), folders.refetch(), savedSearches.refetch(), offline.refetch(), offlineProjects.refetch()])} />
    <header className="mobile-header"><div><h1>{title}</h1><p>{typeof total === "number" && total >= 0 ? `${total} ${t("projects")}` : t("Your private collection")}</p></div><div className="mobile-header__actions"><button className="round-button" aria-label={t("Open project scanner")} onClick={() => { setProjectScanError(""); setProjectScannerOpen(true); }}><QrCode /></button></div></header>
    {usingOfflineFallback && <div className="offline-fallback-banner" role="status"><CloudCheck /><span><strong>{t("Showing saved projects")}</strong><small>{t("Volum Desktop is unavailable. Search and basic filters apply to this device’s offline snapshots.")}</small></span></div>}
    <div className="library-search-row"><form className="search-field" onSubmit={(event) => { event.preventDefault(); setSearch(searchValue.trim()); }}><Search size={18} /><input value={searchValue} onChange={(event) => setSearchValue(event.target.value)} placeholder={t("Search projects")} /><button aria-label={t("Search")}><ChevronRight size={18} /></button></form><button id="library-filter" className={`filter-button ${filterCount ? "is-active" : ""}`} aria-label={t("Filter and sort")} onClick={() => setFiltersOpen(true)}><SlidersHorizontal />{filterCount > 0 && <span>{filterCount}</span>}</button></div>
    {tab === "library" && !search && <section className="collection-strip"><button className={`is-single-line ${!selectedCollection && !folderId && !savedSearchId && !recentOnly ? "is-active" : ""}`} onClick={clearScope}><LibraryBig /><span>{t("All projects")}</span></button><button className={`is-single-line ${recentOnly ? "is-active" : ""}`} onClick={() => { setFolderId(""); setSavedSearchId(""); setRecentOnly(true); onCollection(undefined); }}><Clock3 /><span>{t("Recent")}</span></button>{collections.data?.map((collection) => <button className={selectedCollection === collection.id ? "is-active" : ""} key={collection.id} onClick={() => { setFolderId(""); setSavedSearchId(""); setRecentOnly(false); onCollection(collection.id); }} style={{ "--collection-color": collection.color } as React.CSSProperties}><Boxes /><span>{collection.name}</span><small>{collection.modelCount}</small></button>)}</section>}
    {(selectedCollection || folderId || savedSearchId) && <section className="scope-offline"><div><CloudDownload /><span><strong>{t("Keep this view offline")}</strong><small>{packBusy ? t("{completed} of {total} projects", { completed: packProgress.completed, total: packProgress.total || "…" }) : t("Download every project in this collection or folder.")}</small></span></div><button disabled={packBusy} onClick={() => void saveScopeOffline()}>{packBusy ? <LoaderCircle className="spin" /> : t("Download")}</button>{packBusy && packProgress.total > 0 && <div className="upload-progress"><span style={{ width: `${packProgress.completed / packProgress.total * 100}%` }} /></div>}{packError && <div className="scope-offline__error" role="alert"><CircleAlert />{packError}</div>}</section>}
    {projects.isLoading ? <div className="project-grid skeleton-grid">{Array.from({ length: 8 }, (_, index) => <div className="skeleton-card" key={index}><i /><span /><small /></div>)}</div> : projects.isError && !usingOfflineFallback ? <ErrorState error={projects.error} retrying={projects.isFetching} onRetry={() => void projects.refetch()} /> : visibleItems.length ? <><div className="project-grid">{visibleItems.map((project) => <ProjectCard key={project.id} credential={credential} project={project} offline={offlineIds.has(project.id)} onOpen={() => onOpen(project.id)} />)}</div>{!usingOfflineFallback && projects.hasNextPage && <button className="load-more" disabled={projects.isFetchingNextPage} onClick={() => void projects.fetchNextPage()}>{projects.isFetchingNextPage ? <LoaderCircle className="spin" /> : t("Load more projects")}</button>}</> : <div className="center-state"><Search size={27} /><strong>{t("No projects found")}</strong><p>{t("Try another search, folder, or filter.")}</p></div>}
    {filtersOpen && <div className="sheet-scrim" role="presentation" onClick={() => setFiltersOpen(false)}><section className="mobile-sheet mobile-sheet--tall filter-sheet" role="dialog" aria-modal="true" aria-labelledby="filters-title" onClick={(event) => event.stopPropagation()}><header><div><div className="eyebrow">{t("BROWSE")}</div><h2 id="filters-title">{t("Filter & sort")}</h2></div><button className="round-button" aria-label={t("Close filters")} onClick={() => setFiltersOpen(false)}><X /></button></header><div className="sheet-field-grid"><label className="sheet-field sheet-field--wide"><span>{t("Folder")}</span><select value={folderId} onChange={(event) => { setFolderId(event.target.value); if (event.target.value) { onCollection(undefined); setSavedSearchId(""); setRecentOnly(false); } }}><option value="">{t("All folders")}</option>{folders.data?.map((folder) => <option key={folder.id} value={folder.id}>{folder.name} · {folder.modelCount}</option>)}</select></label><label className="sheet-field sheet-field--wide"><span>{t("Saved search")}</span><select value={savedSearchId} onChange={(event) => { setSavedSearchId(event.target.value); if (event.target.value) { setFolderId(""); setRecentOnly(false); onCollection(undefined); } }}><option value="">{t("None")}</option>{savedSearches.data?.map((saved) => <option key={saved.id} value={saved.id}>{saved.name}</option>)}</select></label><label className="sheet-field"><span>{t("Format")}</span><select value={format} onChange={(event) => setFormat(event.target.value)}><option value="">{t("All")}</option>{["3mf", "stl", "obj", "step"].map((value) => <option key={value} value={value}>{value.toUpperCase()}</option>)}</select></label><label className="sheet-field"><span>{t("Desktop file")}</span><select value={availability} onChange={(event) => setAvailability(event.target.value)}><option value="">{t("Any")}</option><option value="available">{t("Available")}</option><option value="offline">{t("Missing")}</option></select></label><label className="sheet-field sheet-field--wide"><span>{t("Tag")}</span><select value={tagId} onChange={(event) => setTagId(event.target.value)}><option value="">{t("All tags")}</option>{tags.data?.map((tag) => <option key={tag.id} value={tag.id}>{tag.name}</option>)}</select></label><label className="sheet-field sheet-field--wide"><span>{t("Sort")}</span><select value={sort} onChange={(event) => setSort(event.target.value as typeof sort)}><option value="modified">{t("Recently modified")}</option><option value="added">{t("Recently added")}</option><option value="opened">{t("Last opened")}</option><option value="name">{t("Name A–Z")}</option></select></label></div><div className="filter-sheet__actions"><button className="button button--quiet" onClick={() => { setFolderId(""); setSavedSearchId(""); setRecentOnly(false); setFormat(""); setAvailability(""); setTagId(""); setSort("modified"); }}>{t("Clear")}</button><button className="button button--primary" onClick={() => setFiltersOpen(false)}><Check />{t("Show projects")}</button></div></section></div>}
    {projectScannerOpen && !projectScanBusy && <div className="sheet-scrim" role="presentation" onClick={() => void closeProjectScanner()}><section className="mobile-sheet project-scanner-sheet" role="dialog" aria-modal="true" aria-labelledby="project-scanner-title" onClick={(event) => event.stopPropagation()}><header><div><div className="eyebrow">{t("PROJECT QR")}</div><h2 id="project-scanner-title">{t("Open a shared project")}</h2></div><button className="round-button" aria-label={t("Close")} onClick={() => void closeProjectScanner()}><X /></button></header><div className="project-scanner-sheet__icon"><QrCode /></div><p>{t("Scan a project QR shown by Volum Desktop or another device paired with this same Desktop.")}</p><small>{t("This does not pair the app with Volum Desktop. Pairing codes are scanned from the connection screen.")}</small>{projectScanError && <div className="error-banner" role="alert"><CircleAlert />{projectScanError}</div>}<button className="button button--primary button--large" onClick={() => void scanProject()}><QrCode />{t("Scan project QR")}</button></section></div>}
    {projectScannerOpen && projectScanBusy && <QrScannerOverlay title={t("Scan a project QR")} instructions={t("Point the camera at a project code shown in Volum Desktop or another paired Volum device.")} onCancel={() => void closeProjectScanner()} />}
  </main>;
}
