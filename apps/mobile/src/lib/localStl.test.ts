import { describe, expect, it } from "vitest";
import { parseLocalPreviewEnvelope } from "./localStl";

function previewEnvelope() {
  const envelope = new ArrayBuffer(8 + 36 + 9 * 4 + 3 * 4 + 3 + 1);
  const bytes = new Uint8Array(envelope);
  bytes.set(Array.from("VLP1").map((value) => value.charCodeAt(0)));
  const view = new DataView(envelope);
  view.setUint32(4, 1, true);
  bytes.set(Array.from("VLM4").map((value) => value.charCodeAt(0)), 8);
  view.setUint32(12, 3, true);
  view.setUint32(16, 3, true);
  [10, 20, 30, 25, 50, 32].forEach((value, index) => view.setFloat32(20 + index * 4, value, true));
  return envelope;
}

function selectablePreviewEnvelope() {
  const objectCount = 2;
  const triangleCount = 3;
  const headerLength = 40 + objectCount * 32;
  const vertexCount = 9;
  const indexCount = 9;
  const payloadLength = headerLength + vertexCount * 12 + indexCount * 4 + indexCount + triangleCount + triangleCount * 4;
  const envelope = new ArrayBuffer(8 + payloadLength);
  const bytes = new Uint8Array(envelope);
  const view = new DataView(envelope);
  bytes.set(new TextEncoder().encode("VLP1"), 0);
  view.setUint32(4, triangleCount, true);
  bytes.set(new TextEncoder().encode("VLM5"), 8);
  view.setUint32(12, vertexCount, true);
  view.setUint32(16, indexCount, true);
  [0, 0, 0, 30, 20, 10].forEach((value, index) => view.setFloat32(20 + index * 4, value, true));
  view.setUint32(44, objectCount, true);
  view.setUint32(48, 1, true);
  view.setUint32(52, 2, true);
  [0, 0, 0, 10, 20, 10].forEach((value, index) => view.setFloat32(56 + index * 4, value, true));
  view.setUint32(80, 2, true);
  view.setUint32(84, 1, true);
  [20, 5, 0, 30, 15, 5].forEach((value, index) => view.setFloat32(88 + index * 4, value, true));
  return envelope;
}

describe("local STL preview envelopes", () => {
  it("returns source dimensions and the VLM payload", () => {
    const preview = parseLocalPreviewEnvelope(previewEnvelope());
    expect(preview.triangleCount).toBe(1);
    expect(preview.dimensions).toEqual([15, 30, 2]);
    expect(String.fromCharCode(...new Uint8Array(preview.payload, 0, 4))).toBe("VLM4");
    expect(preview.objects).toEqual([]);
  });

  it("returns selectable object metadata from VLM5", () => {
    const preview = parseLocalPreviewEnvelope(selectablePreviewEnvelope());
    expect(preview.objects).toEqual([
      {
        id: 1,
        triangleCount: 2,
        bounds: [0, 0, 0, 10, 20, 10],
        dimensions: [10, 20, 10],
        center: [5, 10, 5],
      },
      {
        id: 2,
        triangleCount: 1,
        bounds: [20, 5, 0, 30, 15, 5],
        dimensions: [10, 10, 5],
        center: [25, 10, 2.5],
      },
    ]);
  });

  it("rejects malformed envelopes", () => {
    expect(() => parseLocalPreviewEnvelope(new ArrayBuffer(16))).toThrow("invalid");
  });
});
