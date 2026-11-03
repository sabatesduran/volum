import type { InfiniteData, QueryClient, QueryKey } from "@tanstack/react-query";
import type { CompanionPage, ProjectDetail, ProjectSummary, Tag } from "../types";

export interface OptimisticProjectChanges {
  displayName?: string;
  favorite?: boolean;
  notes?: string;
  tagIds?: string[];
}

export function applyOptimisticProjectChanges(
  project: ProjectDetail,
  changes: OptimisticProjectChanges,
  availableTags: Tag[],
): ProjectDetail {
  const tagsById = new Map(availableTags.map((tag) => [tag.id, tag]));
  return {
    ...project,
    displayName: changes.displayName ?? project.displayName,
    favorite: changes.favorite ?? project.favorite,
    notes: changes.notes ?? project.notes,
    tags: changes.tagIds == null
      ? project.tags
      : changes.tagIds.flatMap((id) => tagsById.get(id) ?? []),
  };
}

function replaceProjectInPages(
  data: InfiniteData<CompanionPage<ProjectSummary>> | undefined,
  project: ProjectDetail,
) {
  if (!data) return data;
  return {
    ...data,
    pages: data.pages.map((page) => ({
      ...page,
      items: page.items.map((item) => item.id === project.id ? {
        ...item,
        displayName: project.displayName,
        favorite: project.favorite,
        modifiedAt: project.modifiedAt,
        missing: project.missing,
        assetCount: project.assetCount,
        coverPhotoId: project.coverPhotoId,
      } : item),
    })),
  };
}

export async function optimisticProjectMutation(
  queryClient: QueryClient,
  projectId: string,
  optimistic: ProjectDetail,
  mutation: () => Promise<ProjectDetail>,
) {
  const detailKey: QueryKey = ["project", projectId];
  await Promise.all([
    queryClient.cancelQueries({ queryKey: detailKey }),
    queryClient.cancelQueries({ queryKey: ["projects"] }),
  ]);
  const previousDetail = queryClient.getQueryData<ProjectDetail>(detailKey);
  const previousPages = queryClient.getQueriesData<InfiniteData<CompanionPage<ProjectSummary>>>({ queryKey: ["projects"] });
  queryClient.setQueryData(detailKey, optimistic);
  for (const [key, data] of previousPages) {
    queryClient.setQueryData(key, replaceProjectInPages(data, optimistic));
  }
  try {
    const updated = await mutation();
    queryClient.setQueryData(detailKey, updated);
    for (const [key] of previousPages) {
      queryClient.setQueryData<InfiniteData<CompanionPage<ProjectSummary>>>(key, (data) => replaceProjectInPages(data, updated));
    }
    return updated;
  } catch (reason) {
    queryClient.setQueryData(detailKey, previousDetail);
    for (const [key, data] of previousPages) queryClient.setQueryData(key, data);
    throw reason;
  }
}
