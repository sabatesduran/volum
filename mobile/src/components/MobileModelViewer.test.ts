import { describe, expect, it } from "vitest";
import * as THREE from "three";
import { normalizeObject, parseViewerMesh } from "./MobileModelViewer";

function trianglePayload(version: "VLM1" | "VLM2" = "VLM1") {
  const colorBytes = version === "VLM2" ? 9 : 0;
  const buffer = new ArrayBuffer(12 + 9 * 4 + 3 * 4 + colorBytes);
  const bytes = new Uint8Array(buffer);
  bytes.set(Array.from(version).map((character) => character.charCodeAt(0)));
  const view = new DataView(buffer);
  view.setUint32(4, 3, true);
  view.setUint32(8, 3, true);
  [100, 200, 300, 110, 200, 300, 100, 220, 300].forEach((value, index) => view.setFloat32(12 + index * 4, value, true));
  [0, 1, 2].forEach((value, index) => view.setUint32(48 + index * 4, value, true));
  if (version === "VLM2") bytes.set([63, 141, 67, 63, 141, 67, 63, 141, 67], 60);
  return buffer;
}

describe("mobile model viewer geometry", () => {
  it("parses and centers packed geometry", () => {
    const object = normalizeObject(parseViewerMesh(trianglePayload()));
    object.updateMatrixWorld(true);
    const center = new THREE.Box3().setFromObject(object).getCenter(new THREE.Vector3());
    expect(center.length()).toBeLessThan(0.0001);
  });

  it("preserves packed model colors", () => {
    const object = parseViewerMesh(trianglePayload("VLM2"));
    const mesh = object.children[0] as THREE.Mesh;
    expect((mesh.material as THREE.MeshStandardMaterial).color.getHexString()).toBe("3f8d43");
  });

  it("rejects truncated geometry", () => {
    expect(() => parseViewerMesh(new ArrayBuffer(8))).toThrow("incomplete");
  });
});
