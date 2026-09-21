import { Canvas, useFrame, useThree } from "@react-three/fiber";
import { LoaderCircle } from "lucide-react";
import { Suspense, useCallback, useEffect, useMemo, useState } from "react";
import * as THREE from "three";
import { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";
import { api } from "../lib/tauri/api";
import { t } from "../lib/i18n";

export type ModelView = "iso" | "front" | "side" | "top";
export interface ViewerGeometry {
  dimensionsMm: [number, number, number];
  triangleCount: number;
}

const VIEW_POSITIONS: Record<ModelView, [number, number, number]> = {
  iso: [3.2, 2.35, 4.1],
  front: [0, 0, 5],
  side: [5, 0, 0],
  top: [0, 5, 0.001]
};

function Controls({ resetSignal, view }: { resetSignal: number; view: ModelView }) {
  const { camera, gl, invalidate } = useThree();
  const controls = useMemo(() => new OrbitControls(camera, gl.domElement), [camera, gl.domElement]);
  useEffect(() => {
    const onChange = () => invalidate();
    controls.enableDamping = true;
    controls.dampingFactor = .08;
    controls.minDistance = 1.5;
    controls.maxDistance = 12;
    controls.addEventListener("change", onChange);
    return () => { controls.removeEventListener("change", onChange); controls.dispose(); };
  }, [controls, invalidate]);
  useEffect(() => {
    const [x, y, z] = VIEW_POSITIONS[view];
    camera.up.set(0, view === "top" ? 0 : 1, view === "top" ? -1 : 0);
    camera.position.set(x, y, z);
    controls.target.set(0, 0, 0);
    controls.update();
    invalidate();
  }, [camera, controls, invalidate, resetSignal, view]);
  useFrame(() => controls.update());
  return null;
}

export function normalizeObject(object: THREE.Object3D): THREE.Object3D {
  object.updateMatrixWorld(true);
  const box = new THREE.Box3().setFromObject(object);
  if (box.isEmpty()) throw new Error(t("The model contains no displayable geometry."));
  const center = box.getCenter(new THREE.Vector3());
  const size = box.getSize(new THREE.Vector3());
  const largest = Math.max(size.x, size.y, size.z);
  if (!center.toArray().every(Number.isFinite) || !Number.isFinite(largest) || largest <= 0) {
    throw new Error(t("The model has invalid geometry bounds."));
  }

  // Keep centering inside a child group so scale and z-up rotation are applied
  // around the model center rather than around the source file's plate offset.
  object.position.sub(center);
  const normalized = new THREE.Group();
  normalized.add(object);
  normalized.scale.setScalar((object.userData.volumHasBuildPlate ? 1.95 : 2.4) / largest);
  normalized.rotation.x = -Math.PI / 2;
  normalized.traverse((child) => {
    if (child instanceof THREE.Mesh) {
      if (!child.geometry.getAttribute("normal")) child.geometry.computeVertexNormals();
      if (!child.userData.volumSourceMaterial) {
        const previous = child.material;
        child.material = new THREE.MeshStandardMaterial({
          color: "#d8c8aa",
          roughness: .58,
          metalness: .04,
          side: THREE.DoubleSide
        });
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

export function parseViewerMesh(buffer: ArrayBuffer): THREE.Object3D {
  if (buffer.byteLength < 12) throw new Error(t("The viewer received an incomplete model."));
  const header = new Uint8Array(buffer, 0, 4);
  const version = String.fromCharCode(...header);
  if (version !== "VLM1" && version !== "VLM2") throw new Error(t("The viewer received an unsupported model payload."));
  const data = new DataView(buffer);
  const vertexCount = data.getUint32(4, true);
  const indexCount = data.getUint32(8, true);
  const indexOffset = 12 + vertexCount * 3 * 4;
  const colorOffset = indexOffset + indexCount * 4;
  const expectedLength = colorOffset + (version === "VLM2" ? vertexCount * 3 : 0);
  if (vertexCount === 0 || indexCount === 0 || expectedLength !== buffer.byteLength) {
    throw new Error(t("The viewer received invalid model geometry."));
  }
  const positions = new Float32Array(buffer, 12, vertexCount * 3);
  const indices = new Uint32Array(buffer, indexOffset, indexCount);
  if (version === "VLM1") {
    const geometry = new THREE.BufferGeometry();
    geometry.setAttribute("position", new THREE.BufferAttribute(positions, 3));
    geometry.setIndex(new THREE.BufferAttribute(indices, 1));
    geometry.computeVertexNormals();
    geometry.computeBoundingBox();
    geometry.computeBoundingSphere();
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
  const group = new THREE.Group();
  Array.from(layers.values()).forEach((layer, layerIndex) => {
    const geometry = new THREE.BufferGeometry();
    geometry.setAttribute("position", new THREE.BufferAttribute(positions, 3));
    geometry.setIndex(layer.indices);
    geometry.computeVertexNormals();
    geometry.computeBoundingBox();
    geometry.computeBoundingSphere();
    const hex = `#${layer.color.map((value) => value.toString(16).padStart(2, "0")).join("")}`;
    const isPlateSurface = hex === "#363740";
    const isPlateGrid = hex === "#535560" || hex === "#6c6f7b" || hex === "#898c97";
    const isBuildPlate = isPlateSurface || isPlateGrid;
    const material = new THREE.MeshStandardMaterial({
      color: hex,
      emissive: hex,
      emissiveIntensity: isBuildPlate ? .08 : .12,
      roughness: isBuildPlate ? .9 : .58,
      metalness: isBuildPlate ? .08 : .025,
      side: isPlateSurface ? THREE.FrontSide : THREE.DoubleSide,
      polygonOffset: !isBuildPlate && layerIndex > 0,
      polygonOffsetFactor: -layerIndex,
      polygonOffsetUnits: -layerIndex
    });
    const mesh = new THREE.Mesh(geometry, material);
    mesh.userData.volumSourceMaterial = true;
    if (isBuildPlate) {
      mesh.userData.volumBuildPlate = true;
      group.userData.volumHasBuildPlate = true;
    }
    mesh.renderOrder = layerIndex;
    group.add(mesh);
  });
  return group;
}

function disposeObject(object?: THREE.Object3D) {
  object?.traverse((child) => {
    if (child instanceof THREE.Mesh) {
      child.geometry.dispose();
      if (Array.isArray(child.material)) child.material.forEach((material) => material.dispose());
      else if (child.material instanceof THREE.Material) child.material.dispose();
    }
  });
}

function LoadedObject({ assetId, extension, plateIndex, onError, onReady }: { assetId: string; extension: string; plateIndex?: number; onError: (message: string) => void; onReady: (geometry: ViewerGeometry) => void }) {
  const [object, setObject] = useState<THREE.Object3D>();
  useEffect(() => {
    let disposed = false;
    let loaded: THREE.Object3D | undefined;
    setObject(undefined);
    api.viewerMesh(assetId, plateIndex).then((buffer) => {
      if (disposed || buffer.byteLength === 0) return;
      const parsed = parseViewerMesh(buffer);
      const size = new THREE.Box3().setFromObject(parsed).getSize(new THREE.Vector3());
      let triangleCount = 0;
      parsed.traverse((child) => {
        if (child instanceof THREE.Mesh) triangleCount += (child.geometry.index?.count ?? child.geometry.getAttribute("position").count) / 3;
      });
      loaded = normalizeObject(parsed);
      if (disposed) {
        disposeObject(loaded);
        return;
      }
      setObject(loaded);
      onReady({ dimensionsMm: [size.x, size.y, size.z], triangleCount });
    }).catch((error: unknown) => {
      if (!disposed) onError(error instanceof Error ? error.message : String(error));
    });
    return () => {
      disposed = true;
      disposeObject(loaded);
    };
  }, [assetId, extension, onError, onReady, plateIndex]);
  return object ? <primitive object={object} /> : null;
}

function DemoObject({ modelId }: { modelId: string }) {
  const index = Number(modelId.match(/\d+/)?.[0] ?? 1) % 4;
  const material = <meshStandardMaterial color="#d8c8aa" roughness={.52} metalness={.03} />;
  if (index === 0) return <group rotation={[.1, .25, 0]}><mesh position={[0, -.52, 0]}><cylinderGeometry args={[.72, .88, .18, 6]} />{material}</mesh><mesh position={[0, .02, 0]}><cylinderGeometry args={[.38, .7, .95, 6]} />{material}</mesh><mesh position={[0, .6, 0]}><torusGeometry args={[.37, .08, 12, 6]} />{material}</mesh></group>;
  if (index === 1) return <group rotation={[.15, -.3, -.08]}>{[-.55, 0, .55].map((x) => <mesh key={x} position={[x, 0, 0]}><torusKnotGeometry args={[.34, .095, 80, 12, 2, 3]} />{material}</mesh>)}</group>;
  if (index === 2) return <mesh rotation={[0, .3, 0]}><icosahedronGeometry args={[1, 2]} />{material}</mesh>;
  return <group rotation={[.2, -.25, 0]}><mesh><boxGeometry args={[1.7, .22, 1.2]} />{material}</mesh><mesh position={[-.65, .45, 0]}><boxGeometry args={[.22, .8, 1.2]} />{material}</mesh><mesh position={[.65, .45, 0]}><boxGeometry args={[.22, .8, 1.2]} />{material}</mesh></group>;
}

export function ModelViewer({ assetId, extension, modelId, plateIndex, resetSignal = 0, view = "iso", onError, onGeometry }: { assetId?: string; extension: string; modelId: string; plateIndex?: number; resetSignal?: number; view?: ModelView; onError?: (message: string) => void; onGeometry?: (geometry: ViewerGeometry) => void }) {
  const demo = modelId.startsWith("demo-");
  const [status, setStatus] = useState<"loading" | "ready" | "error">(demo || !assetId ? "ready" : "loading");
  useEffect(() => setStatus(demo || !assetId ? "ready" : "loading"), [assetId, demo, plateIndex]);
  const handleError = useCallback((message: string) => {
    setStatus("error");
    onError?.(message);
  }, [onError]);
  const handleReady = useCallback((geometry: ViewerGeometry) => {
    setStatus("ready");
    onError?.("");
    onGeometry?.(geometry);
  }, [onError, onGeometry]);
  return (
    <>
      <Canvas dpr={[1, 1.75]} camera={{ position: VIEW_POSITIONS.iso, fov: 34 }} frameloop="demand" gl={{ antialias: true, alpha: true }}>
        <ambientLight intensity={1.05} />
        <hemisphereLight intensity={1.1} color="#fffaf0" groundColor="#aaa397" />
        <directionalLight position={[4, 6, 5]} intensity={2.35} />
        <directionalLight position={[-4, 2, -3]} intensity={1.05} color="#dce9ff" />
        <Suspense fallback={null}>
          {demo || !assetId ? <DemoObject modelId={modelId} /> : <LoadedObject assetId={assetId} extension={extension} plateIndex={plateIndex} onError={handleError} onReady={handleReady} />}
        </Suspense>
        <Controls resetSignal={resetSignal} view={view} />
      </Canvas>
      {status === "loading" && <div className="viewer-loading"><LoaderCircle className="spin" size={18} /><span>{t("Loading 3D model…")}</span></div>}
    </>
  );
}
