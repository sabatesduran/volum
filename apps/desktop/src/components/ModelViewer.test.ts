import { describe, expect, it } from "vitest";
import * as THREE from "three";
import { BAMBU_LIGHTING, BAMBU_PLATE_COLORS } from "../lib/viewerMaterials";
import { normalizeObject, parseViewerMesh, viewerObjectBounds } from "./ModelViewer";

function uniformRgb(material: THREE.ShaderMaterial) {
  return Array.from(material.uniforms.uniform_color.value as Float32Array)
    .map((channel) => Math.round(channel * 255));
}

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

function coloredTrianglePayload(color: [number, number, number] = [63, 141, 67], version: "VLM2" | "VLM3" = "VLM2") {
  const buffer = new ArrayBuffer(12 + 9 * 4 + 3 * 4 + (version === "VLM2" ? 9 : 3));
  const bytes = new Uint8Array(buffer);
  bytes.set(Array.from(version).map((character) => character.charCodeAt(0)));
  const view = new DataView(buffer);
  view.setUint32(4, 3, true);
  view.setUint32(8, 3, true);
  [0, 0, 0, 10, 0, 0, 0, 10, 0].forEach((value, index) => view.setFloat32(12 + index * 4, value, true));
  [0, 1, 2].forEach((value, index) => view.setUint32(48 + index * 4, value, true));
  bytes.set(version === "VLM2" ? [...color, ...color, ...color] : color, 60);
  return buffer;
}

function multicolorTrianglePayload() {
  const buffer = new ArrayBuffer(12 + 12 * 4 + 6 * 4 + 6);
  const bytes = new Uint8Array(buffer);
  bytes.set(Array.from("VLM3").map((character) => character.charCodeAt(0)));
  const view = new DataView(buffer);
  view.setUint32(4, 4, true);
  view.setUint32(8, 6, true);
  [0, 0, 0, 10, 0, 0, 0, 10, 0, 10, 10, 0].forEach((value, index) => view.setFloat32(12 + index * 4, value, true));
  [0, 1, 2, 1, 3, 2].forEach((value, index) => view.setUint32(60 + index * 4, value, true));
  bytes.set([255, 0, 0, 0, 255, 0], 84);
  return buffer;
}

function semanticPlatePayload() {
  const buffer = new ArrayBuffer(36 + 18 * 4 + 6 * 4 + 6 + 2);
  const bytes = new Uint8Array(buffer);
  bytes.set(Array.from("VLM4").map((character) => character.charCodeAt(0)));
  const view = new DataView(buffer);
  view.setUint32(4, 6, true);
  view.setUint32(8, 6, true);
  [10, 20, 30, 20, 30, 30].forEach((value, index) => view.setFloat32(12 + index * 4, value, true));
  [
    10, 20, 30, 20, 20, 30, 10, 30, 30,
    0, 0, 29, 50, 0, 29, 0, 50, 29,
  ].forEach((value, index) => view.setFloat32(36 + index * 4, value, true));
  [0, 1, 2, 3, 4, 5].forEach((value, index) => view.setUint32(108 + index * 4, value, true));
  bytes.set([54, 55, 64, 54, 55, 64], 132);
  bytes.set([0, 1], 138);
  return buffer;
}

function selectableObjectPayload(names: string[] = []) {
  const objectCount = 2;
  const vertexCount = 9;
  const indexCount = 9;
  const triangleCount = 3;
  const headerLength = 40 + objectCount * 32;
  const encodedNames = names.map((name) => new TextEncoder().encode(name));
  const nameTrailerLength = names.length ? 8 + encodedNames.reduce((total, name) => total + 4 + name.length, 0) : 0;
  const buffer = new ArrayBuffer(headerLength + vertexCount * 12 + indexCount * 4 + indexCount + triangleCount + triangleCount * 4 + nameTrailerLength);
  const bytes = new Uint8Array(buffer);
  const view = new DataView(buffer);
  bytes.set(new TextEncoder().encode("VLM5"));
  view.setUint32(4, vertexCount, true);
  view.setUint32(8, indexCount, true);
  [0, 0, 0, 30, 10, 2].forEach((value, index) => view.setFloat32(12 + index * 4, value, true));
  view.setUint32(36, objectCount, true);
  view.setUint32(40, 1, true);
  view.setUint32(44, 1, true);
  [0, 0, 0, 10, 10, 2].forEach((value, index) => view.setFloat32(48 + index * 4, value, true));
  view.setUint32(72, 2, true);
  view.setUint32(76, 1, true);
  [20, 0, 0, 30, 10, 2].forEach((value, index) => view.setFloat32(80 + index * 4, value, true));
  let offset = headerLength;
  [
    0, 0, 0, 10, 0, 0, 0, 10, 2,
    20, 0, 0, 30, 0, 0, 20, 10, 2,
    -5, -5, -1, 35, -5, -1, -5, 15, -1,
  ].forEach((value) => { view.setFloat32(offset, value, true); offset += 4; });
  [0, 1, 2, 3, 4, 5, 6, 7, 8].forEach((value) => { view.setUint32(offset, value, true); offset += 4; });
  bytes.set([216, 200, 170, 216, 200, 170, 54, 55, 64], offset);
  offset += 9;
  bytes.set([0, 0, 1], offset);
  offset += 3;
  [1, 2, 0].forEach((value) => { view.setUint32(offset, value, true); offset += 4; });
  if (names.length) {
    bytes.set(new TextEncoder().encode("VNM1"), offset);
    view.setUint32(offset + 4, objectCount, true);
    offset += 8;
    encodedNames.forEach((name) => {
      view.setUint32(offset, name.length, true);
      offset += 4;
      bytes.set(name, offset);
      offset += name.length;
    });
  }
  return buffer;
}

describe("model viewer geometry", () => {
  it("parses packed geometry and centers off-origin models", () => {
    const object = normalizeObject(parseViewerMesh(trianglePayload()));
    object.updateMatrixWorld(true);
    const center = new THREE.Box3().setFromObject(object).getCenter(new THREE.Vector3());
    expect(center.length()).toBeLessThan(0.0001);
    const mesh = object.getObjectByProperty("type", "Mesh") as THREE.Mesh;
    expect(mesh.geometry.getAttribute("normal")).toBeUndefined();
    expect((mesh.material as THREE.ShaderMaterial).fragmentShader).toContain("dFdx(eye_position)");
  });

  it("passes packed 3MF colors to the shader as raw RGB", () => {
    const object = parseViewerMesh(coloredTrianglePayload());
    const mesh = object.children[0] as THREE.Mesh;
    const material = mesh.material as THREE.ShaderMaterial;
    expect(material).toBeInstanceOf(THREE.ShaderMaterial);
    expect(uniformRgb(material)).toEqual([63, 141, 67]);
    expect(material.toneMapped).toBe(false);
    expect(material.fragmentShader).not.toContain("colorspace_fragment");
  });

  it("preserves the exact Kalandra yellow in VLM3", () => {
    const object = parseViewerMesh(coloredTrianglePayload([246, 218, 90], "VLM3"));
    const mesh = object.children[0] as THREE.Mesh;
    expect(uniformRgb(mesh.material as THREE.ShaderMaterial)).toEqual([246, 218, 90]);
  });

  it("lifts near-black filament colors and keeps Bambu's subtle specular", () => {
    const object = parseViewerMesh(coloredTrianglePayload([0, 0, 0], "VLM3"));
    const mesh = object.children[0] as THREE.Mesh;
    const material = mesh.material as THREE.ShaderMaterial;
    expect(uniformRgb(material)).toEqual([51, 51, 51]);
    expect(material.uniforms.light_top_specular.value).toBe(BAMBU_LIGHTING.topSpecular);
    expect(material.uniforms.light_top_shininess.value).toBe(BAMBU_LIGHTING.shininess);
  });

  it("uses Bambu Studio's exact three-light constants", () => {
    const object = parseViewerMesh(coloredTrianglePayload([246, 218, 90], "VLM3"));
    const material = (object.children[0] as THREE.Mesh).material as THREE.ShaderMaterial;
    expect(material.uniforms.intensity_ambient.value).toBe(0.3);
    expect(material.uniforms.light_top_diffuse.value).toBe(0.48);
    expect(material.uniforms.light_front_diffuse.value).toBe(0.18);
    expect(material.uniforms.light_back_diffuse.value).toBe(0.18);
    expect(Array.from(material.uniforms.light_top_dir.value as Float32Array)).toEqual(
      expect.arrayContaining(BAMBU_LIGHTING.topDirection.map((value) => expect.closeTo(value, 6))),
    );
  });

  it("renders different colors on triangles that share vertices", () => {
    const object = parseViewerMesh(multicolorTrianglePayload());
    const mesh = object.children[0] as THREE.Mesh;
    const materials = mesh.material as THREE.ShaderMaterial[];
    expect(materials.map(uniformRgb)).toEqual([[255, 0, 0], [0, 255, 0]]);
    expect(mesh.geometry.groups).toHaveLength(2);
    expect(materials.every((material) => material.depthTest && material.depthWrite && !material.polygonOffset)).toBe(true);
  });

  it("keeps VLM4 model semantics and source bounds separate from its plate", () => {
    const object = parseViewerMesh(semanticPlatePayload());
    const mesh = object.children[0] as THREE.Mesh;
    const materials = mesh.material as THREE.ShaderMaterial[];
    expect(materials.map((material) => material.userData.volumMaterialKind)).toEqual([
      "filament",
      "plate-surface",
    ]);
    expect(object.userData.volumHasBuildPlate).toBe(true);
    const bounds = object.userData.volumSourceBounds as THREE.Box3;
    expect(bounds.min.toArray()).toEqual([10, 20, 30]);
    expect(bounds.max.toArray()).toEqual([20, 30, 30]);
  });

  it("maps selectable VLM5 triangles to rendered plate objects", () => {
    const object = parseViewerMesh(selectableObjectPayload());
    const mesh = object.children[0] as THREE.Mesh;
    expect(object.userData.volumObjects).toEqual([
      { id: 1, triangleCount: 1, bounds: [0, 0, 0, 10, 10, 2], dimensions: [10, 10, 2], center: [5, 5, 1] },
      { id: 2, triangleCount: 1, bounds: [20, 0, 0, 30, 10, 2], dimensions: [10, 10, 2], center: [25, 5, 1] },
    ]);
    expect(Array.from(mesh.userData.volumObjectIds as Uint32Array)).toEqual([1, 2, 0]);
    expect(Array.from(mesh.geometry.getAttribute("volum_object_id").array)).toEqual([1, 1, 1, 2, 2, 2, 0, 0, 0]);
    expect(mesh.geometry.boundsTree).toBeDefined();
  });

  it("reads optional 3MF object names from VLM5", () => {
    const object = parseViewerMesh(selectableObjectPayload(["Dock", "Right rail"]));
    expect(object.userData.volumObjects).toEqual([
      expect.objectContaining({ id: 1, name: "Dock" }),
      expect.objectContaining({ id: 2, name: "Right rail" }),
    ]);
  });

  it("transforms rendered object bounds into viewer coordinates", () => {
    const object = normalizeObject(parseViewerMesh(selectableObjectPayload()));
    const bounds = viewerObjectBounds(object, 2)!;
    expect(bounds.isEmpty()).toBe(false);
    expect(bounds.getSize(new THREE.Vector3()).toArray().every((value) => value > 0)).toBe(true);
  });

  it("uses a flat Bambu-colored plate with no halo and hides it from below", () => {
    const object = normalizeObject(parseViewerMesh(coloredTrianglePayload([54, 55, 64])));
    const mesh = object.getObjectByProperty("type", "Mesh") as THREE.Mesh;
    const material = mesh.material as THREE.ShaderMaterial;
    expect(material.side).toBe(THREE.FrontSide);
    expect(material.userData.volumMaterialKind).toBe("plate-surface");
    expect(uniformRgb(material)).toEqual(BAMBU_PLATE_COLORS.surface);
    expect(material.fragmentShader).not.toContain("smoothstep");
    expect(mesh.castShadow).toBe(false);
    expect(mesh.receiveShadow).toBe(false);
  });

  it("keeps the plate grid visible from both sides", () => {
    const object = parseViewerMesh(coloredTrianglePayload([83, 85, 96]));
    const mesh = object.children[0] as THREE.Mesh;
    const material = mesh.material as THREE.ShaderMaterial;
    expect(material.side).toBe(THREE.DoubleSide);
    expect(uniformRgb(material)).toEqual(BAMBU_PLATE_COLORS.grid);
  });
});
