export interface CacheEntry {
  key: string;
  byteSize: number;
  lastAccessedAt: number;
}

export function lruRemovals<T extends CacheEntry>(
  entries: readonly T[],
  maximumBytes: number,
  maximumEntries: number,
): T[] {
  const ordered = [...entries].sort((left, right) =>
    left.lastAccessedAt - right.lastAccessedAt || left.key.localeCompare(right.key));
  let byteSize = ordered.reduce((total, entry) => total + entry.byteSize, 0);
  let count = ordered.length;
  const removals: T[] = [];
  for (const entry of ordered) {
    if (byteSize <= maximumBytes && count <= maximumEntries) break;
    removals.push(entry);
    byteSize -= entry.byteSize;
    count -= 1;
  }
  return removals;
}

export class PromiseCache<T> {
  private readonly values = new Map<string, Promise<T>>();

  constructor(private readonly maximumEntries: number) {}

  get(key: string, loader: () => Promise<T>): Promise<T> {
    const cached = this.values.get(key);
    if (cached) return cached;
    const request = loader().catch((error) => {
      if (this.values.get(key) === request) this.values.delete(key);
      throw error;
    });
    this.values.set(key, request);
    while (this.values.size > this.maximumEntries) {
      const oldest = this.values.keys().next().value as string | undefined;
      if (oldest !== undefined) this.values.delete(oldest);
    }
    return request;
  }

  delete(key: string) {
    this.values.delete(key);
  }

  clear() {
    this.values.clear();
  }
}
