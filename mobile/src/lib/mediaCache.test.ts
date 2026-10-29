import "fake-indexeddb/auto";
import { afterEach, describe, expect, it } from "vitest";
import type { Credential } from "../types";
import type { MediaFetcher } from "./mediaCache";
import {
  clearMediaCache,
  mediaCacheStats,
  pruneMediaCache,
  requestCachedBlob,
} from "./mediaCache";

const servers = new Set<string>();

function credential(serverId: string): Credential {
  servers.add(serverId);
  return {
    serverId,
    serverName: "Desktop",
    baseUrl: "http://100.64.0.1:43821",
    deviceId: "device",
    token: "token",
  };
}

function bytes(...values: number[]) {
  return new Uint8Array(values).buffer;
}

afterEach(async () => {
  await Promise.all([...servers].map(clearMediaCache));
  servers.clear();
});

describe("transient media cache", () => {
  it("revalidates cached media with its ETag", async () => {
    const desktop = credential("server-a");
    const seenEtags: Array<string | undefined> = [];
    const fetcher: MediaFetcher = async (_credential, _path, etag) => {
      seenEtags.push(etag);
      if (etag) return { status: 304, etag, contentType: "image/png", bytes: bytes() };
      return { status: 200, etag: "\"one\"", contentType: "image/png", bytes: bytes(1, 2, 3) };
    };

    const first = await requestCachedBlob(desktop, "/cover", "image/png", fetcher);
    const second = await requestCachedBlob(desktop, "/cover", "image/png", fetcher);

    expect(seenEtags).toEqual([undefined, "\"one\""]);
    expect([...new Uint8Array(await first.arrayBuffer())]).toEqual([1, 2, 3]);
    expect([...new Uint8Array(await second.arrayBuffer())]).toEqual([1, 2, 3]);
    expect(await mediaCacheStats("server-a")).toEqual({ count: 1, byteSize: 3 });
  });

  it("replaces bytes when the server returns a new ETag", async () => {
    const desktop = credential("server-a");
    let revision = 1;
    const fetcher: MediaFetcher = async () => ({
      status: 200,
      etag: `\"${revision}\"`,
      contentType: "image/png",
      bytes: bytes(revision++),
    });

    await requestCachedBlob(desktop, "/cover", "image/png", fetcher);
    const updated = await requestCachedBlob(desktop, "/cover", "image/png", fetcher);

    expect([...new Uint8Array(await updated.arrayBuffer())]).toEqual([2]);
  });

  it("uses stale bytes only for connection failures", async () => {
    const desktop = credential("server-a");
    await requestCachedBlob(desktop, "/cover", "image/png", async () => ({
      status: 200,
      etag: "\"one\"",
      contentType: "image/png",
      bytes: bytes(7),
    }));

    const stale = await requestCachedBlob(desktop, "/cover", "image/png", async () => {
      throw new Error("Could not reach Volum Desktop through Tailscale.");
    });
    expect([...new Uint8Array(await stale.arrayBuffer())]).toEqual([7]);

    await expect(requestCachedBlob(desktop, "/cover", "image/png", async () => {
      throw new Error("This device is no longer paired with Volum Desktop");
    })).rejects.toThrow("no longer paired");
  });

  it("evicts least-recently-used entries to both byte and count limits", async () => {
    const desktop = credential("server-a");
    const fetcher: MediaFetcher = async (_credential, path) => ({
      status: 200,
      etag: `\"${path}\"`,
      contentType: "image/png",
      bytes: bytes(1, 2, 3, 4),
    });
    await requestCachedBlob(desktop, "/a", "image/png", fetcher);
    await requestCachedBlob(desktop, "/b", "image/png", fetcher);

    const result = await pruneMediaCache(4, 1);

    expect(result).toEqual({ byteSize: 4, count: 1, removed: 1 });
    expect(await mediaCacheStats("server-a")).toEqual({ count: 1, byteSize: 4 });
  });

  it("clears only media belonging to the unpaired Desktop", async () => {
    const first = credential("server-a");
    const second = credential("server-b");
    const fetcher: MediaFetcher = async () => ({
      status: 200,
      etag: "\"one\"",
      contentType: "image/png",
      bytes: bytes(1),
    });
    await requestCachedBlob(first, "/cover", "image/png", fetcher);
    await requestCachedBlob(second, "/cover", "image/png", fetcher);

    await clearMediaCache("server-a");

    expect(await mediaCacheStats("server-a")).toEqual({ count: 0, byteSize: 0 });
    expect(await mediaCacheStats("server-b")).toEqual({ count: 1, byteSize: 1 });
  });
});
