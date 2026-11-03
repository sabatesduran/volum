import * as THREE from "three";
import { acceleratedRaycast, computeBoundsTree, disposeBoundsTree } from "three-mesh-bvh";
import {
  BAMBU_PLATE_COLORS,
  createBambuMaterial,
  createPlateSurfaceMaterial,
  createRawPlateMaterial,
  type ViewerRgb,
} from "./materials";

export type ViewerTranslate = (message: string) => string;
const identity: ViewerTranslate = (message) => message;

THREE.BufferGeometry.prototype.computeBoundsTree = computeBoundsTree;
THREE.BufferGeometry.prototype.disposeBoundsTree = disposeBoundsTree;
THREE.Mesh.prototype.raycast = acceleratedRaycast;

export type ModelView = "iso" | "front" | "side" | "top";
export interface ViewerGeometry {
  dimensionsMm: [number, number, number];
  triangleCount: number;
}
export interface ViewerObject {
  id: number;
  name?: string;
  triangleCount: number;
  bounds: [number, number, number, number, number, number];
  dimensions: [number, number, number];
  center: [number, number, number];
}

const buildPlateColors = new Map<string, ViewerRgb>([
  ["#363740", BAMBU_PLATE_COLORS.surface],
  ["#535560", BAMBU_PLATE_COLORS.grid],
  ["#6c6f7b", BAMBU_PLATE_COLORS.majorGrid],
  ["#898c97", BAMBU_PLATE_COLORS.border],
]);

function colorHex(color: [number, number, number]) {
  return `#${color.map((value) => value.toString(16).padStart(2, "0")).join("")}`;
}

function viewerObjectNames(buffer: ArrayBuffer, offset: number, objectCount: number, translate: ViewerTranslate) {
  if (offset === buffer.byteLength) return [];
  if (buffer.byteLength - offset < 8) throw new Error(translate("The viewer received invalid model geometry."));
  const bytes = new Uint8Array(buffer);
  const trailerVersion = String.fromCharCode(...bytes.subarray(offset, offset + 4));
  const data = new DataView(buffer);
  if (trailerVersion !== "VNM1" || data.getUint32(offset + 4, true) !== objectCount) {
    throw new Error(translate("The viewer received invalid model geometry."));
  }
  const decoder = new TextDecoder("utf-8", { fatal: true });
  const names: string[] = [];
  let cursor = offset + 8;
  try {
    for (let index = 0; index < objectCount; index += 1) {
      if (cursor + 4 > buffer.byteLength) throw new Error();
      const length = data.getUint32(cursor, true);
      cursor += 4;
      if (length > 1_024 || cursor + length > buffer.byteLength) throw new Error();
      names.push(decoder.decode(bytes.subarray(cursor, cursor + length)).trim());
      cursor += length;
    }
  } catch {
    throw new Error(translate("The viewer received invalid model geometry."));
  }
  if (cursor !== buffer.byteLength) throw new Error(translate("The viewer received invalid model geometry."));
  return names;
}

export function visibleObjectSphere(object: THREE.Object3D) {
  const bounds = new THREE.Box3();
  object.updateWorldMatrix(true, true);
  object.traverse((child) => {
    if (!(child instanceof THREE.Mesh) || !child.visible) return;
    const materials = Array.isArray(child.material) ? child.material : [child.material];
    if (materials.length && materials.every((material) => !material.visible)) return;
    if (!child.geometry.boundingBox) child.geometry.computeBoundingBox();
    if (!child.geometry.boundingBox) return;
    bounds.union(child.geometry.boundingBox.clone().applyMatrix4(child.matrixWorld));
  });
  if (bounds.isEmpty()) return undefined;
  return bounds.getBoundingSphere(new THREE.Sphere());
}
export function normalizeObject(object: THREE.Object3D, modelOnly = false, translate: ViewerTranslate = identity): THREE.Object3D {
  object.updateMatrixWorld(true);
  const sourceBounds = object.userData.volumSourceBounds;
  const box = modelOnly && sourceBounds instanceof THREE.Box3
    ? sourceBounds.clone()
    : new THREE.Box3().setFromObject(object);
  if (box.isEmpty()) throw new Error(translate("The model contains no displayable geometry."));
  const center = box.getCenter(new THREE.Vector3());
  const size = box.getSize(new THREE.Vector3());
  const largest = Math.max(size.x, size.y, size.z);
  if (!center.toArray().every(Number.isFinite) || !Number.isFinite(largest) || largest <= 0) {
    throw new Error(translate("The model has invalid geometry bounds."));
  }

  // Keep centering inside a child group so scale and z-up rotation are applied
  // around the model center rather than around the source file's plate offset.
  object.position.sub(center);
  const normalized = new THREE.Group();
  normalized.add(object);
  normalized.scale.setScalar((object.userData.volumHasBuildPlate && !modelOnly ? 1.95 : 2.4) / largest);
  normalized.rotation.x = -Math.PI / 2;
  normalized.userData.volumObjects = object.userData.volumObjects;
  normalized.userData.volumSourceBounds = object.userData.volumSourceBounds;
  normalized.userData.volumSourceObject = object;
  normalized.traverse((child) => {
    if (child instanceof THREE.Mesh) {
      const sourceMaterials = Array.isArray(child.material) ? child.material : [child.material];
      if (modelOnly) sourceMaterials.forEach((material) => {
        if (material.userData.volumMaterialKind === "plate-surface" || material.userData.volumMaterialKind === "plate-detail") material.visible = false;
      });
      if (!child.userData.volumSourceMaterial) {
        const previous = child.material;
        child.material = createBambuMaterial([216, 200, 170]);
        if (Array.isArray(previous)) previous.forEach((material) => material.dispose());
        else if (previous instanceof THREE.Material) previous.dispose();
      }
      child.castShadow = false;
      child.receiveShadow = false;
    }
  });
  normalized.updateMatrixWorld(true);
  return normalized;
}

export function viewerObjectBounds(object: THREE.Object3D, objectId: number) {
  const objects = object.userData.volumObjects as ViewerObject[] | undefined;
  const source = object.userData.volumSourceObject as THREE.Object3D | undefined;
  const metadata = objects?.find((candidate) => candidate.id === objectId);
  if (!source || !metadata) return undefined;
  object.updateMatrixWorld(true);
  const [minX, minY, minZ, maxX, maxY, maxZ] = metadata.bounds;
  const output = new THREE.Box3();
  for (const x of [minX, maxX]) {
    for (const y of [minY, maxY]) {
      for (const z of [minZ, maxZ]) {
        output.expandByPoint(new THREE.Vector3(x, y, z).applyMatrix4(source.matrixWorld));
      }
    }
  }
  return output;
}

export function parseViewerMesh(buffer: ArrayBuffer, translate: ViewerTranslate = identity): THREE.Object3D {
  if (buffer.byteLength < 12) throw new Error(translate("The viewer received an incomplete model."));
  const version = String.fromCharCode(...new Uint8Array(buffer, 0, 4));
  if (version !== "VLM1" && version !== "VLM2" && version !== "VLM3" && version !== "VLM4" && version !== "VLM5") throw new Error(translate("The viewer received an unsupported model payload."));
  const data = new DataView(buffer);
  const vertexCount = data.getUint32(4, true);
  const indexCount = data.getUint32(8, true);
  const objectCount = version === "VLM5"
    ? buffer.byteLength >= 40
      ? data.getUint32(36, true)
      : 4_097
    : 0;
  const headerLength = version === "VLM5"
    ? 40 + objectCount * 32
    : version === "VLM4"
      ? 36
      : 12;
  const indexOffset = headerLength + vertexCount * 3 * 4;
  const colorOffset = indexOffset + indexCount * 4;
  const colorLength = version === "VLM2" ? vertexCount * 3 : version === "VLM3" || version === "VLM4" || version === "VLM5" ? indexCount : 0;
  const kindOffset = colorOffset + colorLength;
  const kindLength = version === "VLM4" || version === "VLM5" ? indexCount / 3 : 0;
  const objectIdOffset = kindOffset + kindLength;
  const objectIdLength = version === "VLM5" ? indexCount / 3 * 4 : 0;
  const geometryLength = objectIdOffset + objectIdLength;
  if (!vertexCount || !indexCount || indexCount % 3 !== 0 || objectCount > 4_096 || headerLength > buffer.byteLength || geometryLength > buffer.byteLength
    || (version !== "VLM5" && geometryLength !== buffer.byteLength)) {
    throw new Error(translate("The viewer received invalid model geometry."));
  }
  const objectNames = version === "VLM5" ? viewerObjectNames(buffer, geometryLength, objectCount, translate) : [];

  const objects: ViewerObject[] = [];
  if (version === "VLM5") {
    let sourceTriangles = 0;
    for (let index = 0; index < objectCount; index += 1) {
      const offset = 40 + index * 32;
      const id = data.getUint32(offset, true);
      const triangleCount = data.getUint32(offset + 4, true);
      const bounds = Array.from({ length: 6 }, (_, bound) => data.getFloat32(offset + 8 + bound * 4, true)) as ViewerObject["bounds"];
      if (id !== index + 1 || !triangleCount || !bounds.every(Number.isFinite)
        || bounds[3] < bounds[0] || bounds[4] < bounds[1] || bounds[5] < bounds[2]) {
        throw new Error(translate("The viewer received invalid model geometry."));
      }
      objects.push({
        id,
        ...(objectNames[index] ? { name: objectNames[index] } : {}),
        triangleCount,
        bounds,
        dimensions: [bounds[3] - bounds[0], bounds[4] - bounds[1], bounds[5] - bounds[2]],
        center: [(bounds[0] + bounds[3]) * 0.5, (bounds[1] + bounds[4]) * 0.5, (bounds[2] + bounds[5]) * 0.5],
      });
      sourceTriangles += triangleCount;
    }
    if (objects.length && sourceTriangles > indexCount / 3) throw new Error(translate("The viewer received invalid model geometry."));
  }

  const positions = new Float32Array(buffer, headerLength, vertexCount * 3);
  const indices = new Uint32Array(buffer, indexOffset, indexCount);
  if (version === "VLM1") {
    const geometry = new THREE.BufferGeometry();
    geometry.setAttribute("position", new THREE.BufferAttribute(positions, 3));
    geometry.setAttribute("volum_object_id", new THREE.BufferAttribute(new Float32Array(vertexCount), 1));
    geometry.setIndex(new THREE.BufferAttribute(indices, 1));
    geometry.computeBoundingBox();
    geometry.computeBoundingSphere();
    return new THREE.Mesh(geometry);
  }
  const colors = new Uint8Array(buffer, colorOffset, colorLength);
  const kinds = version === "VLM4" || version === "VLM5" ? new Uint8Array(buffer, kindOffset, kindLength) : undefined;
  const sourceObjectIds = version === "VLM5" ? new Uint32Array(buffer, objectIdOffset, indexCount / 3) : undefined;
  const layers = new Map<string, { color: [number, number, number]; kind: number; indices: number[]; objectIds: number[] }>();
  for (let offset = 0; offset < indices.length; offset += 3) {
    const vertex = indices[offset];
    const triangleColorOffset = version === "VLM3" || version === "VLM4" || version === "VLM5" ? offset : vertex * 3;
    const color: [number, number, number] = [colors[triangleColorOffset], colors[triangleColorOffset + 1], colors[triangleColorOffset + 2]];
    const kind = kinds?.[offset / 3] ?? 0;
    const objectId = sourceObjectIds?.[offset / 3] ?? 0;
    if (kind > 4) throw new Error(translate("The viewer received invalid model geometry."));
    if (version === "VLM5" && (objectId > objectCount || (kind === 0 && objectCount > 0 && objectId === 0) || (kind !== 0 && objectId !== 0))) {
      throw new Error(translate("The viewer received invalid model geometry."));
    }
    const key = `${kind}:${color.join(",")}`;
    const layer = layers.get(key) ?? { color, kind, indices: [], objectIds: [] };
    layer.indices.push(indices[offset], indices[offset + 1], indices[offset + 2]);
    layer.objectIds.push(objectId);
    layers.set(key, layer);
  }
  const geometry = new THREE.BufferGeometry();
  geometry.setAttribute("position", new THREE.BufferAttribute(positions, 3));
  const vertexObjectIds = new Float32Array(vertexCount);
  const reordered = new Uint32Array(indexCount);
  const reorderedObjectIds = new Uint32Array(indexCount / 3);
  const materials: THREE.Material[] = [];
  let indexStart = 0;
  let hasBuildPlate = false;
  const orderedLayers = Array.from(layers.values());
  orderedLayers.forEach((layer, layerIndex) => {
    reordered.set(layer.indices, indexStart);
    reorderedObjectIds.set(layer.objectIds, indexStart / 3);
    layer.objectIds.forEach((objectId, triangle) => {
      const triangleOffset = triangle * 3;
      vertexObjectIds[layer.indices[triangleOffset]] = objectId;
      vertexObjectIds[layer.indices[triangleOffset + 1]] = objectId;
      vertexObjectIds[layer.indices[triangleOffset + 2]] = objectId;
    });
    const hex = colorHex(layer.color);
    const semanticKinds = version === "VLM4" || version === "VLM5";
    const legacyPlateColor = semanticKinds ? undefined : buildPlateColors.get(hex);
    const isPlateSurface = semanticKinds ? layer.kind === 1 : hex === "#363740";
    const plateColor = semanticKinds
      ? layer.kind === 2
        ? BAMBU_PLATE_COLORS.grid
        : layer.kind === 3
          ? BAMBU_PLATE_COLORS.majorGrid
          : layer.kind === 4
            ? BAMBU_PLATE_COLORS.border
            : undefined
      : legacyPlateColor;
    const isBuildPlate = plateColor != null;
    const material = isPlateSurface
      ? createPlateSurfaceMaterial()
      : plateColor
        ? createRawPlateMaterial(plateColor)
        : createBambuMaterial(layer.color);
    materials.push(material);
    geometry.addGroup(indexStart, layer.indices.length, layerIndex);
    indexStart += layer.indices.length;
    hasBuildPlate ||= isPlateSurface || isBuildPlate;
  });
  geometry.setAttribute("volum_object_id", new THREE.BufferAttribute(vertexObjectIds, 1));
  geometry.setIndex(new THREE.BufferAttribute(reordered, 1));
  geometry.computeBoundingBox();
  geometry.computeBoundingSphere();
  if (version === "VLM5" && objects.length) geometry.computeBoundsTree({ indirect: true, maxLeafTris: 24 });
  const mesh = new THREE.Mesh(geometry, materials.length === 1 ? materials[0] : materials);
  mesh.userData.volumSourceMaterial = true;
  mesh.userData.volumObjectIds = reorderedObjectIds;
  mesh.renderOrder = layers.size;
  const group = new THREE.Group();
  group.userData.volumHasBuildPlate = hasBuildPlate;
  group.userData.volumObjects = objects;
  if (version === "VLM4" || version === "VLM5") {
    const bounds = Array.from({ length: 6 }, (_, index) => data.getFloat32(12 + index * 4, true));
    if (!bounds.every(Number.isFinite) || bounds[3] < bounds[0] || bounds[4] < bounds[1] || bounds[5] < bounds[2]) {
      throw new Error(translate("The viewer received invalid model geometry."));
    }
    group.userData.volumSourceBounds = new THREE.Box3(
      new THREE.Vector3(bounds[0], bounds[1], bounds[2]),
      new THREE.Vector3(bounds[3], bounds[4], bounds[5]),
    );
  }
  group.add(mesh);
  return group;
}
