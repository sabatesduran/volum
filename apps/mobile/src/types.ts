export * from "@volum/companion-api/types";

export interface OfflineScope {
  key: `${string}:${"collection" | "folder" | "saved-search"}:${string}`;
  serverId: string;
  kind: "collection" | "folder" | "saved-search";
  scopeId: string;
  label: string;
  params: string;
  projectIds: string[];
  syncedAt: string;
  libraryRevision?: string;
  error?: string;
}
