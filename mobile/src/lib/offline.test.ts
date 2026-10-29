import "fake-indexeddb/auto";
import { afterEach, describe, expect, it } from "vitest";
import type { OfflineProjectSnapshot } from "../types";
import {
  clearOfflineProjects,
  getOfflineProject,
  listOfflineManifests,
  removeOfflineProject,
  storeOfflineProject,
} from "./offline";

const servers = new Set<string>();
const databaseName = "volum-companion-offline";

function snapshot(serverId: string, projectId: string, syncedAt: string): OfflineProjectSnapshot {
  servers.add(serverId);
  const revision = `${syncedAt}-revision`;
  return {
    manifest: {
      schemaVersion: 2,
      key: `${serverId}:${projectId}`,
      serverId,
      projectId,
      displayName: `Project ${projectId}`,
      revision,
      syncedAt,
      byteSize: 128,
      mediaCount: 1,
    },
    detail: {
      id: projectId,
      displayName: `Project ${projectId}`,
      folderId: "folder",
      folderName: "Folder",
      primaryExtension: "stl",
      favorite: false,
      addedAt: syncedAt,
      modifiedAt: syncedAt,
      missing: false,
      assetCount: 0,
      revision,
      notes: "",
      rootName: "Library",
      assets: [],
      collectionIds: [],
      tags: [],
    },
    documentation: {
      revision,
      photos: [],
      customValues: [],
      bomItems: [],
      assemblySteps: [],
    },
    photoThumbnails: [],
  };
}

afterEach(async () => {
  await Promise.all([...servers].map(clearOfflineProjects));
  servers.clear();
});

function deleteOfflineDatabase() {
  return new Promise<void>((resolve, reject) => {
    const request = indexedDB.deleteDatabase(databaseName);
    request.onsuccess = () => resolve();
    request.onerror = () => reject(request.error);
    request.onblocked = () => reject(new Error("Offline database deletion was blocked."));
  });
}

function seedVersionOne(value: OfflineProjectSnapshot) {
  return new Promise<void>((resolve, reject) => {
    const request = indexedDB.open(databaseName, 1);
    request.onupgradeneeded = () => request.result.createObjectStore("projects", { keyPath: "manifest.key" });
    request.onerror = () => reject(request.error);
    request.onsuccess = () => {
      const database = request.result;
      const transaction = database.transaction("projects", "readwrite");
      const legacy = structuredClone(value) as unknown as { manifest: { schemaVersion?: number } };
      delete legacy.manifest.schemaVersion;
      transaction.objectStore("projects").put(legacy);
      transaction.oncomplete = () => { database.close(); resolve(); };
      transaction.onerror = () => reject(transaction.error);
    };
  });
}

describe("offline project repository", () => {
  it("isolates manifests by paired Desktop and sorts newest first", async () => {
    await storeOfflineProject(snapshot("server-a", "older", "2026-01-01T10:00:00Z"));
    await storeOfflineProject(snapshot("server-b", "other", "2026-01-03T10:00:00Z"));
    await storeOfflineProject(snapshot("server-a", "newer", "2026-01-02T10:00:00Z"));

    const manifests = await listOfflineManifests("server-a");
    expect(manifests.map((manifest) => manifest.projectId)).toEqual(["newer", "older"]);
    expect(await getOfflineProject("server-b", "other")).toBeDefined();
  });

  it("removes one project without touching other snapshots", async () => {
    await storeOfflineProject(snapshot("server-a", "one", "2026-01-01T10:00:00Z"));
    await storeOfflineProject(snapshot("server-a", "two", "2026-01-02T10:00:00Z"));

    await removeOfflineProject("server-a", "one");

    expect(await getOfflineProject("server-a", "one")).toBeUndefined();
    expect(await getOfflineProject("server-a", "two")).toBeDefined();
  });

  it("clears only the unpaired Desktop cache", async () => {
    await storeOfflineProject(snapshot("server-a", "one", "2026-01-01T10:00:00Z"));
    await storeOfflineProject(snapshot("server-b", "two", "2026-01-02T10:00:00Z"));

    await clearOfflineProjects("server-a");

    expect(await listOfflineManifests("server-a")).toEqual([]);
    expect(await listOfflineManifests("server-b")).toHaveLength(1);
  });

  it("rejects a snapshot whose key does not match its identities", async () => {
    const invalid = snapshot("server-a", "one", "2026-01-01T10:00:00Z");
    invalid.manifest.key = "server-a:other";
    await expect(storeOfflineProject(invalid)).rejects.toThrow("identity is invalid");
  });

  it("migrates version-one manifests when the cache opens", async () => {
    await deleteOfflineDatabase();
    const legacy = snapshot("server-a", "legacy", "2026-01-01T10:00:00Z");
    await seedVersionOne(legacy);

    const migrated = await getOfflineProject("server-a", "legacy");

    expect(migrated?.manifest.schemaVersion).toBe(2);
  });
});
