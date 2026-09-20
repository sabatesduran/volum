import { describe, expect, it } from "vitest";
import * as THREE from "three";
import { normalizeObject, parseViewerMesh } from "./ModelViewer";

function trianglePayload() {
  const buffer = new ArrayBuffer(12 + 9 * 4 + 3 * 4);
  const bytes = new Uint8Array(buffer);
  bytes.set([86, 76, 77, 49]);
  const view = new DataView(buffer);
  view.setUint32(4, 3, true);
  view.setUint32(8, 3, true);
  [100, 200, 300, 110, 200, 300, 100, 220, 300].forEach((value, index) => view.setFloat32(12 + index * 4, value, true));
  [0, 1, 2].forEach((value, index) => view.setUint32(48 + index * 4, value, true));
  return buffer;
}

function coloredTrianglePayload(color: [number, number, number] = [63, 141, 67]) {
  const buffer = new ArrayBuffer(12 + 9 * 4 + 3 * 4 + 9);
  const bytes = new Uint8Array(buffer);
  bytes.set([86, 76, 77, 50]);
  const view = new DataView(buffer);
  view.setUint32(4, 3, true);
  view.setUint32(8, 3, true);
  [0, 0, 0, 10, 0, 0, 0, 10, 0].forEach((value, index) => view.setFloat32(12 + index * 4, value, true));
  [0, 1, 2].forEach((value, index) => view.setUint32(48 + index * 4, value, true));
  bytes.set([...color, ...color, ...color], 60);
  return buffer;
}

describe("model viewer geometry", () => {
  it("parses packed geometry and centers off-origin models", () => {
    const object = normalizeObject(parseViewerMesh(trianglePayload()));
    object.updateMatrixWorld(true);
    const center = new THREE.Box3().setFromObject(object).getCenter(new THREE.Vector3());
    expect(center.length()).toBeLessThan(0.0001);
    const mesh = object.getObjectByProperty("type", "Mesh") as THREE.Mesh;
    expect(mesh.geometry.getAttribute("normal").count).toBe(3);
  });

  it("preserves packed 3MF vertex colors", () => {
    const object = parseViewerMesh(coloredTrianglePayload());
    const mesh = object.children[0] as THREE.Mesh;
    expect((mesh.material as THREE.MeshStandardMaterial).color.getHexString()).toBe("3f8d43");
  });

  it("hides plate surfaces from below and disables shadows", () => {
    const object = normalizeObject(parseViewerMesh(coloredTrianglePayload([54, 55, 64])));
    const mesh = object.getObjectByProperty("type", "Mesh") as THREE.Mesh;
    const material = mesh.material as THREE.MeshStandardMaterial;
    expect(material.side).toBe(THREE.FrontSide);
    expect(mesh.castShadow).toBe(false);
    expect(mesh.receiveShadow).toBe(false);
  });

  it("keeps the plate grid visible from both sides", () => {
    const object = parseViewerMesh(coloredTrianglePayload([83, 85, 96]));
    const mesh = object.children[0] as THREE.Mesh;
    expect((mesh.material as THREE.MeshStandardMaterial).side).toBe(THREE.DoubleSide);
  });
});
