import { describe, expect, it } from "vitest";
import type { LocalStlObject } from "../lib/localStl";
import { buildObjectPreviewSamples } from "./ObjectPreviewThumbnail";

const objects: LocalStlObject[] = [
  {
    id: 1,
    triangleCount: 2,
    bounds: [0, 0, 0, 1, 1, 1],
    dimensions: [1, 1, 1],
    center: [0.5, 0.5, 0.5],
  },
  {
    id: 2,
    triangleCount: 1,
    bounds: [0, 0, 0, 1, 1, 1],
    dimensions: [1, 1, 1],
    center: [0.5, 0.5, 0.5],
  },
];

function payloadWithObjectIds(objectIds: number[]) {
  const vertexCount = 3;
  const indexCount = objectIds.length * 3;
  const objectCount = objects.length;
  const vertexOffset = 40 + objectCount * 32;
  const indexOffset = vertexOffset + vertexCount * 12;
  const colorOffset = indexOffset + indexCount * 4;
  const kindOffset = colorOffset + objectIds.length * 3;
  const objectIdOffset = kindOffset + objectIds.length;
  const payload = new ArrayBuffer(objectIdOffset + objectIds.length * 4);
  const bytes = new Uint8Array(payload);
  bytes.set(new TextEncoder().encode("VLM5"));
  const view = new DataView(payload);
  view.setUint32(4, vertexCount, true);
  view.setUint32(8, indexCount, true);
  view.setUint32(36, objectCount, true);
  objectIds.forEach((objectId, index) => {
    view.setUint32(objectIdOffset + index * 4, objectId, true);
  });
  return payload;
}

describe("buildObjectPreviewSamples", () => {
  it("groups valid triangles by known object id", () => {
    const samples = buildObjectPreviewSamples(payloadWithObjectIds([1, 1, 2, 99]), objects);
    expect(samples.get(1)).toEqual([0, 1]);
    expect(samples.get(2)).toEqual([2]);
    expect(samples.has(99)).toBe(false);
  });

  it("rejects truncated and implausible payload layouts", () => {
    expect(buildObjectPreviewSamples(new ArrayBuffer(39), objects).size).toBe(0);

    const payload = payloadWithObjectIds([1]);
    new DataView(payload).setUint32(36, 4_097, true);
    expect(buildObjectPreviewSamples(payload, objects).size).toBe(0);
  });
});
