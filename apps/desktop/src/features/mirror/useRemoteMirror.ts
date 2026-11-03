import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useMemo, useState } from "react";
import {
  backendMode,
  currentBackendConnection,
  getRemoteMirrorSettings,
  listMirroredProjects,
  setRemoteMirrorMode,
  setRemoteMirrorSelection,
  syncRemoteMirror,
} from "../../lib/tauri/api";
import type { RemoteMirrorSelection, RemoteMirrorSettings } from "../../types";

function selectionFrom(settings: RemoteMirrorSettings): RemoteMirrorSelection {
  return {
    projectIds: settings.projectIds,
    excludedProjectIds: settings.excludedProjectIds,
    folderIds: settings.folderIds,
    collectionIds: settings.collectionIds,
  };
}

function toggled(values: string[], id: string, enabled: boolean) {
  return enabled ? [...new Set([...values, id])] : values.filter((value) => value !== id);
}

export function useRemoteMirror() {
  const queryClient = useQueryClient();
  const remote = backendMode() === "remote";
  const enabled = remote && Boolean(currentBackendConnection()?.mirrorEnabled);
  const settings = useQuery({
    queryKey: ["remote-mirror-settings"],
    queryFn: getRemoteMirrorSettings,
    enabled: remote,
    staleTime: 30_000,
  });
  const projects = useQuery({
    queryKey: ["mirrored-projects"],
    queryFn: listMirroredProjects,
    enabled,
    retry: false,
  });
  const [busy, setBusy] = useState(false);
  const availableProjectIds = useMemo(
    () => new Set((projects.data ?? [])
      .filter((project) => project.assetCount > 0 && project.assetCount === project.availableAssetCount)
      .map((project) => project.projectId)),
    [projects.data],
  );
  const refresh = async () => {
    await Promise.all([
      queryClient.invalidateQueries({ queryKey: ["remote-mirror-settings"] }),
      queryClient.invalidateQueries({ queryKey: ["mirrored-projects"] }),
      queryClient.invalidateQueries({ queryKey: ["backend-connection"] }),
    ]);
  };
  const applySelection = async (next: RemoteMirrorSelection) => {
    setBusy(true);
    try {
      await setRemoteMirrorSelection(next);
      queryClient.setQueryData<RemoteMirrorSettings>(["remote-mirror-settings"], (current) => current ? { ...current, ...next } : current);
      await syncRemoteMirror();
      await refresh();
    } finally {
      setBusy(false);
    }
  };
  const keepProjects = async (projectIds: string[], keep: boolean) => {
    const current = settings.data;
    if (!current || current.mode !== "selected") return;
    const selected = new Set(current.projectIds);
    const excluded = new Set(current.excludedProjectIds);
    for (const projectId of projectIds) {
      if (keep) {
        selected.add(projectId);
        excluded.delete(projectId);
      } else {
        selected.delete(projectId);
        excluded.add(projectId);
      }
    }
    await applySelection({
      ...selectionFrom(current),
      projectIds: [...selected],
      excludedProjectIds: [...excluded],
    });
  };
  const keepScope = async (kind: "folder" | "collection", id: string, keep: boolean) => {
    const current = settings.data;
    if (!current || current.mode !== "selected") return;
    await applySelection({
      ...selectionFrom(current),
      folderIds: kind === "folder" ? toggled(current.folderIds, id, keep) : current.folderIds,
      collectionIds: kind === "collection" ? toggled(current.collectionIds, id, keep) : current.collectionIds,
    });
  };
  const changeMode = async (mode: "all" | "selected") => {
    setBusy(true);
    try {
      const next = await setRemoteMirrorMode(mode);
      queryClient.setQueryData(["remote-mirror-settings"], next);
      await syncRemoteMirror();
      await refresh();
    } finally {
      setBusy(false);
    }
  };
  const syncNow = async () => {
    setBusy(true);
    try {
      await syncRemoteMirror();
      await refresh();
    } finally {
      setBusy(false);
    }
  };
  return {
    remote,
    enabled,
    settings: settings.data,
    projects: projects.data ?? [],
    availableProjectIds,
    busy,
    keepProjects,
    keepScope,
    changeMode,
    syncNow,
    refresh,
  };
}
