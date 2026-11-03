import { QueryClient } from "@tanstack/react-query";
import { describe, expect, it } from "vitest";
import type { CompanionPage, ProjectDetail, ProjectSummary, Tag } from "../types";
import {
  applyOptimisticProjectChanges,
  optimisticProjectMutation,
} from "./optimisticProject";

function project(overrides: Partial<ProjectDetail> = {}): ProjectDetail {
  return {
    id: "project",
    displayName: "Phone stand",
    folderId: "folder",
    folderName: "Library",
    primaryExtension: "stl",
    favorite: false,
    addedAt: "2026-01-01T00:00:00Z",
    modifiedAt: "2026-01-01T00:00:00Z",
    missing: false,
    assetCount: 0,
    revision: "one",
    notes: "",
    rootName: "Library",
    assets: [],
    collectionIds: [],
    tags: [],
    ...overrides,
  };
}

function page(item: ProjectSummary) {
  return { pages: [{ items: [item], total: 1 } satisfies CompanionPage<ProjectSummary>], pageParams: [""] };
}

describe("optimistic project mutations", () => {
  it("applies editable fields without inventing a server revision", () => {
    const tag: Tag = { id: "tag", name: "Ready", color: "#4f9368", modelCount: 1 };
    const updated = applyOptimisticProjectChanges(project(), {
      displayName: "Desk stand",
      favorite: true,
      notes: "Print four walls",
      tagIds: [tag.id],
    }, [tag]);
    expect(updated).toMatchObject({
      displayName: "Desk stand",
      favorite: true,
      notes: "Print four walls",
      revision: "one",
      tags: [tag],
    });
  });

  it("updates detail and list caches before the request resolves", async () => {
    const client = new QueryClient();
    const original = project();
    const optimistic = project({ favorite: true });
    client.setQueryData(["project", "project"], original);
    client.setQueryData(["projects", "library"], page(original));
    let resolve!: (value: ProjectDetail) => void;
    const response = new Promise<ProjectDetail>((done) => { resolve = done; });
    let markStarted!: () => void;
    const started = new Promise<void>((done) => { markStarted = done; });

    const mutation = optimisticProjectMutation(client, "project", optimistic, () => {
      markStarted();
      return response;
    });
    await started;
    expect(client.getQueryData<ProjectDetail>(["project", "project"])?.favorite).toBe(true);
    expect(client.getQueryData<ReturnType<typeof page>>(["projects", "library"])?.pages[0].items[0].favorite).toBe(true);

    resolve(project({ favorite: true, revision: "two" }));
    await mutation;
    expect(client.getQueryData<ProjectDetail>(["project", "project"])?.revision).toBe("two");
  });

  it("rolls detail and list caches back when the server rejects a write", async () => {
    const client = new QueryClient();
    const original = project();
    client.setQueryData(["project", "project"], original);
    client.setQueryData(["projects", "favorites"], page(original));

    await expect(optimisticProjectMutation(
      client,
      "project",
      project({ favorite: true }),
      async () => { throw new Error("Write permission disabled"); },
    )).rejects.toThrow("Write permission disabled");

    expect(client.getQueryData(["project", "project"])).toEqual(original);
    expect(client.getQueryData<ReturnType<typeof page>>(["projects", "favorites"])?.pages[0].items[0].favorite).toBe(false);
  });
});
