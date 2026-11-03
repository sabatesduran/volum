import { describe, expect, it } from "vitest";
import { lruRemovals, PromiseCache } from "./cache";

describe("cache policies", () => {
  it("evicts oldest entries until both limits are satisfied", () => {
    const entries = [
      { key: "new", byteSize: 7, lastAccessedAt: 3 },
      { key: "old", byteSize: 5, lastAccessedAt: 1 },
      { key: "middle", byteSize: 6, lastAccessedAt: 2 },
    ];
    expect(lruRemovals(entries, 10, 2).map((entry) => entry.key)).toEqual(["old", "middle"]);
  });

  it("deduplicates in-flight work", async () => {
    const cache = new PromiseCache<number>(2);
    let calls = 0;
    const loader = async () => ++calls;
    expect(await Promise.all([cache.get("a", loader), cache.get("a", loader)])).toEqual([1, 1]);
    expect(calls).toBe(1);
  });
});
