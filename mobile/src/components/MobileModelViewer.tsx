import { Canvas, useFrame, useThree } from "@react-three/fiber";
import { Box, CircleAlert, LoaderCircle } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import * as THREE from "three";
import { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";
import { t } from "../lib/i18n";
import { requestCachedBinary } from "../lib/mediaCache";
import type { Credential } from "../types";

const viewerPayloads = new Map<string, Promise<ArrayBuffer>>();

function viewerPayload(credential: Credential, assetId: string, revision: string, plateIndex?: number, offlinePayload?: ArrayBuffer) {
  if (offlinePayload && plateIndex == null) return Promise.resolve(offlinePayload);
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

function Controls({ resetSignal }: { resetSignal: number }) {
  const { camera, gl, invalidate } = useThree();
  const controls = useMemo(() => new OrbitControls(camera, gl.domElement), [camera, gl.domElement]);
  useEffect(() => {
    const onChange = () => invalidate();
    controls.enableDamping = true;
    controls.dampingFactor = 0.08;
    controls.enablePan = false;
    controls.rotateSpeed = 0.72;
    controls.zoomSpeed = 0.85;
    controls.minDistance = 1.5;
    controls.maxDistance = 10;
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
  useFrame(() => controls.update());
  return null;
}

export function parseViewerMesh(buffer: ArrayBuffer) {
  if (buffer.byteLength < 12) throw new Error("The viewer received an incomplete model.");
  const version = String.fromCharCode(...new Uint8Array(buffer, 0, 4));
  if (version !== "VLM1" && version !== "VLM2") throw new Error("The viewer received an unsupported model.");
  const data = new DataView(buffer);
  const vertexCount = data.getUint32(4, true);
  const indexCount = data.getUint32(8, true);
  const indexOffset = 12 + vertexCount * 3 * 4;
  const colorOffset = indexOffset + indexCount * 4;
  const expectedLength = colorOffset + (version === "VLM2" ? vertexCount * 3 : 0);
  if (!vertexCount || !indexCount || expectedLength !== buffer.byteLength) throw new Error("The viewer received invalid model geometry.");

  const positions = new Float32Array(buffer, 12, vertexCount * 3);
  const indices = new Uint32Array(buffer, indexOffset, indexCount);
  if (version === "VLM1") {
    const geometry = new THREE.BufferGeometry();
    geometry.setAttribute("position", new THREE.BufferAttribute(positions, 3));
    geometry.setIndex(new THREE.BufferAttribute(indices, 1));
    geometry.computeVertexNormals();
    return new THREE.Mesh(geometry);
  }

  const colors = new Uint8Array(buffer, colorOffset, vertexCount * 3);
  const layers = new Map<string, { color: [number, number, number]; indices: number[] }>();
  for (let offset = 0; offset < indices.length; offset += 3) {
    const vertex = indices[offset];
    const color: [number, number, number] = [colors[vertex * 3], colors[vertex * 3 + 1], colors[vertex * 3 + 2]];
    const key = color.join(",");
    const layer = layers.get(key) ?? { color, indices: [] };
    layer.indices.push(indices[offset], indices[offset + 1], indices[offset + 2]);
    layers.set(key, layer);
  }
  const geometry = new THREE.BufferGeometry();
  geometry.setAttribute("position", new THREE.BufferAttribute(positions, 3));
  const reordered = new Uint32Array(indexCount);
  const materials: THREE.Material[] = [];
  let indexStart = 0;
  let hasBuildPlate = false;
  Array.from(layers.values()).forEach((layer, layerIndex) => {
    reordered.set(layer.indices, indexStart);
    const hex = `#${layer.color.map((value) => value.toString(16).padStart(2, "0")).join("")}`;
    const plateSurface = hex === "#363740";
    const buildPlate = plateSurface || hex === "#535560" || hex === "#6c6f7b" || hex === "#898c97";
    materials.push(new THREE.MeshStandardMaterial({
      color: hex,
      emissive: hex,
      emissiveIntensity: buildPlate ? 0.08 : 0.12,
      roughness: buildPlate ? 0.9 : 0.58,
      metalness: buildPlate ? 0.08 : 0.025,
      side: plateSurface ? THREE.FrontSide : THREE.DoubleSide,
      polygonOffset: !buildPlate && layerIndex > 0,
      polygonOffsetFactor: -layerIndex,
      polygonOffsetUnits: -layerIndex,
    }));
    geometry.addGroup(indexStart, layer.indices.length, layerIndex);
    indexStart += layer.indices.length;
    hasBuildPlate ||= buildPlate;
  });
  geometry.setIndex(new THREE.BufferAttribute(reordered, 1));
  geometry.computeVertexNormals();
  const mesh = new THREE.Mesh(geometry, materials.length === 1 ? materials[0] : materials);
  mesh.userData.volumSourceMaterial = true;
  const group = new THREE.Group();
  group.userData.volumHasBuildPlate = hasBuildPlate;
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
  normalized.traverse((child) => {
    if (!(child instanceof THREE.Mesh)) return;
    if (!child.geometry.getAttribute("normal")) child.geometry.computeVertexNormals();
    if (!child.userData.volumSourceMaterial) {
      const previous = child.material;
      child.material = new THREE.MeshStandardMaterial({
        color: "#d8c8aa",
        roughness: 0.58,
        metalness: 0.04,
        side: THREE.DoubleSide,
      });
      if (Array.isArray(previous)) previous.forEach((material) => material.dispose());
      else previous.dispose();
    }
    child.castShadow = false;
    child.receiveShadow = false;
  });
  return normalized;
}

function LoadedModel({ credential, assetId, revision, plateIndex, offlinePayload, onStatus }: { credential: Credential; assetId: string; revision: string; plateIndex?: number; offlinePayload?: ArrayBuffer; onStatus: (status: "ready" | "error", message?: string) => void }) {
  const [object, setObject] = useState<THREE.Object3D>();
  useEffect(() => {
    let active = true;
    let loaded: THREE.Object3D | undefined;
    setObject(undefined);
    void viewerPayload(credential, assetId, revision, plateIndex, offlinePayload).then((buffer) => {
      loaded = normalizeObject(parseViewerMesh(buffer));
      if (!active) return;
      setObject(loaded);
      onStatus("ready");
    }).catch((error: unknown) => {
      if (active) onStatus("error", error instanceof Error ? error.message : String(error));
    });
    return () => {
      active = false;
    };
  }, [assetId, credential, offlinePayload, onStatus, plateIndex, revision]);
  return object ? <primitive object={object} /> : null;
}

export function MobileModelViewer({ credential, assetId, revision, plateIndex, offlinePayload, resetSignal, onStatus }: { credential: Credential; assetId: string; revision: string; plateIndex?: number; offlinePayload?: ArrayBuffer; resetSignal: number; onStatus: (status: "ready" | "error", message?: string) => void }) {
  const [status, setStatus] = useState<"loading" | "ready" | "error">("loading");
  const [message, setMessage] = useState("");
  useEffect(() => {
    setStatus("loading");
    setMessage("");
  }, [assetId, plateIndex, revision]);
  const handleStatus = useMemo(() => (next: "ready" | "error", error = "") => {
    setStatus(next);
    setMessage(error);
    onStatus(next, error);
  }, [onStatus]);
  return <div className="mobile-model-viewer">
    <Canvas dpr={[1, 1.5]} camera={{ position: [3.2, 2.35, 4.1], fov: 34 }} frameloop="demand" gl={{ antialias: true, alpha: true, powerPreference: "high-performance" }}>
      <ambientLight intensity={1.05} />
      <hemisphereLight intensity={1.1} color="#fffaf0" groundColor="#aaa397" />
      <directionalLight position={[4, 6, 5]} intensity={2.35} />
      <directionalLight position={[-4, 2, -3]} intensity={1.05} color="#dce9ff" />
      <LoadedModel credential={credential} assetId={assetId} revision={revision} plateIndex={plateIndex} offlinePayload={offlinePayload} onStatus={handleStatus} />
      <Controls resetSignal={resetSignal} />
    </Canvas>
    {status === "loading" && <div className="viewer-state" role="status"><LoaderCircle className="spin" /><span>{t("Preparing 3D model…")}</span></div>}
    {status === "error" && <div className="viewer-state viewer-state--error" role="alert"><CircleAlert /><span>{t(message || "This model could not be displayed.")}</span></div>}
    {status === "ready" && <div className="viewer-hint"><Box />{t("Drag to rotate · Pinch to zoom")}</div>}
  </div>;
}
