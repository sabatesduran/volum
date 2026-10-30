import { Canvas, type ThreeEvent, useFrame, useThree } from "@react-three/fiber";
import { CircleAlert, LoaderCircle } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import * as THREE from "three";
import { acceleratedRaycast, computeBoundsTree, disposeBoundsTree } from "three-mesh-bvh";
import { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";
import { t } from "../lib/i18n";
import { requestCachedBinary } from "../lib/mediaCache";
import {
  BAMBU_PLATE_COLORS,
  createBambuMaterial,
  createPlateSurfaceMaterial,
  createRawPlateMaterial,
  type ViewerRgb,
} from "../lib/viewerMaterials";
import type { Credential } from "../types";
import type { LocalStlObject } from "../lib/localStl";

THREE.BufferGeometry.prototype.computeBoundsTree = computeBoundsTree;
THREE.BufferGeometry.prototype.disposeBoundsTree = disposeBoundsTree;
THREE.Mesh.prototype.raycast = acceleratedRaycast;

const viewerPayloads = new Map<string, Promise<ArrayBuffer>>();
const buildPlateColors = new Map<string, ViewerRgb>([
  ["#363740", BAMBU_PLATE_COLORS.surface],
  ["#535560", BAMBU_PLATE_COLORS.grid],
  ["#6c6f7b", BAMBU_PLATE_COLORS.majorGrid],
  ["#898c97", BAMBU_PLATE_COLORS.border],
]);

function colorHex(color: [number, number, number]) {
  return `#${color.map((value) => value.toString(16).padStart(2, "0")).join("")}`;
}

function viewerObjectNames(buffer: ArrayBuffer, offset: number, objectCount: number) {
  if (offset === buffer.byteLength) return [];
  if (buffer.byteLength - offset < 8) throw new Error("The viewer received invalid model geometry.");
  const bytes = new Uint8Array(buffer);
  const trailerVersion = String.fromCharCode(...bytes.subarray(offset, offset + 4));
  const data = new DataView(buffer);
  if (trailerVersion !== "VNM1" || data.getUint32(offset + 4, true) !== objectCount) {
    throw new Error("The viewer received invalid model geometry.");
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
    throw new Error("The viewer received invalid model geometry.");
  }
  if (cursor !== buffer.byteLength) throw new Error("The viewer received invalid model geometry.");
  return names;
}

function viewerPayload(credential: Credential | undefined, assetId: string | undefined, revision: string, plateIndex?: number, offlinePayload?: ArrayBuffer, localPayload?: ArrayBuffer) {
  if (localPayload) return Promise.resolve(localPayload);
  if (offlinePayload && plateIndex == null) return Promise.resolve(offlinePayload);
  if (!credential || !assetId) return Promise.reject(new Error("The local model preview is unavailable."));
  const key = `${credential.serverId}:${assetId}:${revision}:${plateIndex ?? "all"}`;
  const cached = viewerPayloads.get(key);
  if (cached) return cached;
  const request = requestCachedBinary(
    credential,
    `/api/v1/assets/${encodeURIComponent(assetId)}/viewer-mesh${plateIndex == null ? "" : `?plateIndex=${plateIndex}`}`,
  ).catch((error) => {
    viewerPayloads.delete(key);
    throw error;
  });
  viewerPayloads.set(key, request);
  while (viewerPayloads.size > 3) viewerPayloads.delete(viewerPayloads.keys().next().value!);
  return request;
}

function Controls({ resetSignal, focusBounds, focusSignal }: { resetSignal: number; focusBounds?: THREE.Box3; focusSignal: number }) {
  const { camera, gl, invalidate } = useThree();
  const controls = useMemo(() => new OrbitControls(camera, gl.domElement), [camera, gl.domElement]);
  useEffect(() => {
    const onChange = () => invalidate();
    controls.enableDamping = true;
    controls.dampingFactor = 0.08;
    controls.enablePan = false;
    controls.rotateSpeed = 0.72;
    controls.zoomSpeed = 0.85;
    controls.minDistance = 0.12;
    controls.maxDistance = 12;
    controls.touches.ONE = THREE.TOUCH.ROTATE;
    controls.touches.TWO = THREE.TOUCH.DOLLY_PAN;
    controls.addEventListener("change", onChange);
    return () => {
      controls.removeEventListener("change", onChange);
      controls.dispose();
    };
  }, [controls, invalidate]);
  useEffect(() => {
    camera.up.set(0, 1, 0);
    camera.position.set(3.2, 2.35, 4.1);
    controls.target.set(0, 0, 0);
    controls.update();
    invalidate();
  }, [camera, controls, invalidate, resetSignal]);
  useEffect(() => {
    if (!focusBounds || focusBounds.isEmpty()) return;
    const center = focusBounds.getCenter(new THREE.Vector3());
    const radius = focusBounds.getSize(new THREE.Vector3()).length() * 0.5;
    if (!Number.isFinite(radius) || radius <= 0) return;
    const direction = camera.position.clone().sub(controls.target).normalize();
    if (!direction.lengthSq()) direction.set(0.6, 0.45, 0.7).normalize();
    const verticalFov = camera instanceof THREE.PerspectiveCamera
      ? THREE.MathUtils.degToRad(camera.fov)
      : THREE.MathUtils.degToRad(34);
    const distance = Math.max(radius / Math.sin(verticalFov * 0.5) * 1.18, controls.minDistance * 1.2);
    controls.target.copy(center);
    camera.position.copy(center).addScaledVector(direction, Math.min(distance, controls.maxDistance));
    controls.update();
    invalidate();
  }, [camera, controls, focusBounds, focusSignal, invalidate]);
  useFrame(() => controls.update());
  return null;
}

export function parseViewerMesh(buffer: ArrayBuffer) {
  if (buffer.byteLength < 12) throw new Error("The viewer received an incomplete model.");
  const version = String.fromCharCode(...new Uint8Array(buffer, 0, 4));
  if (version !== "VLM1" && version !== "VLM2" && version !== "VLM3" && version !== "VLM4" && version !== "VLM5") throw new Error("The viewer received an unsupported model.");
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
    || (version !== "VLM5" && geometryLength !== buffer.byteLength)) throw new Error("The viewer received invalid model geometry.");
  const objectNames = version === "VLM5" ? viewerObjectNames(buffer, geometryLength, objectCount) : [];

  const objects: LocalStlObject[] = [];
  if (version === "VLM5") {
    let sourceTriangles = 0;
    for (let index = 0; index < objectCount; index += 1) {
      const offset = 40 + index * 32;
      const id = data.getUint32(offset, true);
      const triangleCount = data.getUint32(offset + 4, true);
      const bounds = Array.from({ length: 6 }, (_, bound) => data.getFloat32(offset + 8 + bound * 4, true)) as LocalStlObject["bounds"];
      if (id !== index + 1 || !triangleCount || !bounds.every(Number.isFinite)
        || bounds[3] < bounds[0] || bounds[4] < bounds[1] || bounds[5] < bounds[2]) {
        throw new Error("The viewer received invalid model geometry.");
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
    if (objects.length && sourceTriangles > indexCount / 3) throw new Error("The viewer received invalid model geometry.");
  }

  const positions = new Float32Array(buffer, headerLength, vertexCount * 3);
  const indices = new Uint32Array(buffer, indexOffset, indexCount);
  if (version === "VLM1") {
    const geometry = new THREE.BufferGeometry();
    geometry.setAttribute("position", new THREE.BufferAttribute(positions, 3));
    geometry.setAttribute("volum_object_id", new THREE.BufferAttribute(new Float32Array(vertexCount), 1));
    geometry.setIndex(new THREE.BufferAttribute(indices, 1));
    return new THREE.Mesh(geometry);
  }

  const colors = new Uint8Array(buffer, colorOffset, colorLength);
  const kinds = version === "VLM4" || version === "VLM5" ? new Uint8Array(buffer, kindOffset, kindLength) : undefined;
  const sourceObjectIds = version === "VLM5" ? new Uint32Array(buffer, objectIdOffset, indexCount / 3) : undefined;
  const layers = new Map<string, { color: [number, number, number]; kind: number; indices: number[]; objectIds: number[] }>();
  for (let offset = 0; offset < indices.length; offset += 3) {
    const vertex = indices[offset];
    const colorOffset = version === "VLM3" || version === "VLM4" || version === "VLM5" ? offset : vertex * 3;
    const color: [number, number, number] = [colors[colorOffset], colors[colorOffset + 1], colors[colorOffset + 2]];
    const kind = kinds?.[offset / 3] ?? 0;
    const objectId = sourceObjectIds?.[offset / 3] ?? 0;
    if (kind > 4) throw new Error("The viewer received invalid model geometry.");
    if (version === "VLM5" && (objectId > objectCount || (kind === 0 && objectCount > 0 && objectId === 0) || (kind !== 0 && objectId !== 0))) {
      throw new Error("The viewer received invalid model geometry.");
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
    const buildPlate = plateColor != null;
    materials.push(isPlateSurface
      ? createPlateSurfaceMaterial()
      : plateColor
        ? createRawPlateMaterial(plateColor)
        : createBambuMaterial(layer.color));
    geometry.addGroup(indexStart, layer.indices.length, layerIndex);
    indexStart += layer.indices.length;
    hasBuildPlate ||= isPlateSurface || buildPlate;
  });
  geometry.setAttribute("volum_object_id", new THREE.BufferAttribute(vertexObjectIds, 1));
  geometry.setIndex(new THREE.BufferAttribute(reordered, 1));
  if (version === "VLM5" && objects.length) geometry.computeBoundsTree({ indirect: true, maxLeafTris: 24 });
  const mesh = new THREE.Mesh(geometry, materials.length === 1 ? materials[0] : materials);
  mesh.userData.volumSourceMaterial = true;
  mesh.userData.volumObjectIds = reorderedObjectIds;
  const group = new THREE.Group();
  group.userData.volumHasBuildPlate = hasBuildPlate;
  group.userData.volumObjects = objects;
  if (version === "VLM4" || version === "VLM5") {
    const bounds = Array.from({ length: 6 }, (_, index) => data.getFloat32(12 + index * 4, true));
    if (!bounds.every(Number.isFinite) || bounds[3] < bounds[0] || bounds[4] < bounds[1] || bounds[5] < bounds[2]) {
      throw new Error("The viewer received invalid model geometry.");
    }
    group.userData.volumSourceBounds = new THREE.Box3(
      new THREE.Vector3(bounds[0], bounds[1], bounds[2]),
      new THREE.Vector3(bounds[3], bounds[4], bounds[5]),
    );
  }
  group.add(mesh);
  return group;
}

export function normalizeObject(object: THREE.Object3D) {
  object.updateMatrixWorld(true);
  const box = new THREE.Box3().setFromObject(object);
  if (box.isEmpty()) throw new Error("The model contains no displayable geometry.");
  const center = box.getCenter(new THREE.Vector3());
  const size = box.getSize(new THREE.Vector3());
  const largest = Math.max(size.x, size.y, size.z);
  if (!Number.isFinite(largest) || largest <= 0) throw new Error("The model has invalid geometry bounds.");
  object.position.sub(center);
  const normalized = new THREE.Group();
  normalized.add(object);
  normalized.scale.setScalar((object.userData.volumHasBuildPlate ? 1.95 : 2.4) / largest);
  normalized.rotation.x = -Math.PI / 2;
  normalized.userData.volumObjects = object.userData.volumObjects;
  normalized.userData.volumSourceBounds = object.userData.volumSourceBounds;
  normalized.userData.volumSourceObject = object;
  normalized.traverse((child) => {
    if (!(child instanceof THREE.Mesh)) return;
    if (!child.userData.volumSourceMaterial) {
      const previous = child.material;
      child.material = createBambuMaterial([216, 200, 170]);
      if (Array.isArray(previous)) previous.forEach((material) => material.dispose());
      else previous.dispose();
    }
    child.castShadow = false;
    child.receiveShadow = false;
  });
  return normalized;
}

export function viewerObjectBounds(object: THREE.Object3D, objectId: number) {
  const objects = object.userData.volumObjects as LocalStlObject[] | undefined;
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

function disposeViewerObject(object: THREE.Object3D) {
  object.traverse((child) => {
    if (!(child instanceof THREE.Mesh)) return;
    child.geometry.disposeBoundsTree?.();
    child.geometry.dispose();
    const materials = Array.isArray(child.material) ? child.material : [child.material];
    materials.forEach((material) => material.dispose());
  });
}

function RaycasterSettings() {
  const raycaster = useThree((state) => state.raycaster);
  useEffect(() => {
    const previous = raycaster.firstHitOnly;
    raycaster.firstHitOnly = true;
    return () => { raycaster.firstHitOnly = previous; };
  }, [raycaster]);
  return null;
}

function LoadedModel({ credential, assetId, revision, plateIndex, offlinePayload, localPayload, selectedObjectId, isolatedObjectId, onObjectSelect, onLoaded, onStatus }: { credential?: Credential; assetId?: string; revision: string; plateIndex?: number; offlinePayload?: ArrayBuffer; localPayload?: ArrayBuffer; selectedObjectId?: number; isolatedObjectId?: number; onObjectSelect?: (objectId?: number) => void; onLoaded: (object?: THREE.Object3D) => void; onStatus: (status: "ready" | "error", message?: string) => void }) {
  const [object, setObject] = useState<THREE.Object3D>();
  const invalidate = useThree((state) => state.invalidate);
  useEffect(() => {
    let active = true;
    let loaded: THREE.Object3D | undefined;
    setObject(undefined);
    onLoaded(undefined);
    void viewerPayload(credential, assetId, revision, plateIndex, offlinePayload, localPayload).then((buffer) => {
      loaded = normalizeObject(parseViewerMesh(buffer));
      if (!active) return;
      setObject(loaded);
      onLoaded(loaded);
      onStatus("ready");
    }).catch((error: unknown) => {
      if (active) onStatus("error", error instanceof Error ? error.message : String(error));
    });
    return () => {
      active = false;
      onLoaded(undefined);
      if (loaded) disposeViewerObject(loaded);
    };
  }, [assetId, credential, localPayload, offlinePayload, onLoaded, onStatus, plateIndex, revision]);
  useEffect(() => {
    if (!object) return;
    object.traverse((child) => {
      if (!(child instanceof THREE.Mesh)) return;
      const materials = Array.isArray(child.material) ? child.material : [child.material];
      materials.forEach((material) => {
        if (!(material instanceof THREE.ShaderMaterial) || !material.uniforms.selected_object_id) return;
        material.uniforms.selected_object_id.value = selectedObjectId ?? 0;
        material.uniforms.isolated_object_id.value = isolatedObjectId ?? 0;
      });
    });
    invalidate();
  }, [invalidate, isolatedObjectId, object, selectedObjectId]);
  const selectObject = (event: ThreeEvent<MouseEvent>) => {
    if (!onObjectSelect || event.delta > 8 || event.faceIndex == null) return;
    const objectIds = event.object.userData.volumObjectIds as Uint32Array | undefined;
    const objectId = objectIds?.[event.faceIndex] ?? 0;
    if (isolatedObjectId && objectId !== isolatedObjectId) return;
    event.stopPropagation();
    onObjectSelect(objectId || undefined);
  };
  return object ? <primitive object={object} onClick={selectObject} /> : null;
}

export function MobileModelViewer({ credential, assetId, revision = "local", plateIndex, offlinePayload, localPayload, resetSignal, selectedObjectId, isolatedObjectId, focusObjectId, focusSignal = 0, onObjectSelect, onObjects, onStatus }: { credential?: Credential; assetId?: string; revision?: string; plateIndex?: number; offlinePayload?: ArrayBuffer; localPayload?: ArrayBuffer; resetSignal: number; selectedObjectId?: number; isolatedObjectId?: number; focusObjectId?: number; focusSignal?: number; onObjectSelect?: (objectId?: number) => void; onObjects?: (objects: LocalStlObject[]) => void; onStatus: (status: "ready" | "error", message?: string) => void }) {
  const [status, setStatus] = useState<"loading" | "ready" | "error">("loading");
  const [message, setMessage] = useState("");
  const [loadedObject, setLoadedObject] = useState<THREE.Object3D>();
  useEffect(() => {
    setStatus("loading");
    setMessage("");
  }, [assetId, localPayload, plateIndex, revision]);
  const handleStatus = useMemo(() => (next: "ready" | "error", error = "") => {
    setStatus(next);
    setMessage(error);
    onStatus(next, error);
  }, [onStatus]);
  const focusBounds = useMemo(
    () => loadedObject && focusObjectId ? viewerObjectBounds(loadedObject, focusObjectId) : undefined,
    [focusObjectId, focusSignal, loadedObject],
  );
  useEffect(() => {
    onObjects?.((loadedObject?.userData.volumObjects as LocalStlObject[] | undefined) ?? []);
  }, [loadedObject, onObjects]);
  return <div className="mobile-model-viewer" onContextMenu={(event) => event.preventDefault()}>
    <Canvas flat dpr={[1, 1.5]} camera={{ position: [3.2, 2.35, 4.1], fov: 34 }} frameloop="demand" gl={{ antialias: true, alpha: true, powerPreference: "high-performance" }} onPointerMissed={() => { if (!isolatedObjectId) onObjectSelect?.(undefined); }}>
      <LoadedModel credential={credential} assetId={assetId} revision={revision} plateIndex={plateIndex} offlinePayload={offlinePayload} localPayload={localPayload} selectedObjectId={selectedObjectId} isolatedObjectId={isolatedObjectId} onObjectSelect={onObjectSelect} onLoaded={setLoadedObject} onStatus={handleStatus} />
      <Controls resetSignal={resetSignal} focusBounds={focusBounds} focusSignal={focusSignal} />
      <RaycasterSettings />
    </Canvas>
    {status === "loading" && <div className="viewer-state" role="status"><LoaderCircle className="spin" /><span>{t("Preparing 3D model…")}</span></div>}
    {status === "error" && <div className="viewer-state viewer-state--error" role="alert"><CircleAlert /><span>{t(message || "This model could not be displayed.")}</span></div>}
  </div>;
}
