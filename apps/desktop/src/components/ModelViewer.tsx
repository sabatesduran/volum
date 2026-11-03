import { Canvas, type ThreeEvent, useFrame, useThree } from "@react-three/fiber";
import { LoaderCircle } from "lucide-react";
import { Suspense, useCallback, useEffect, useMemo, useState } from "react";
import * as THREE from "three";
import { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";
import { photoRoomCameraFitDistance } from "@volum/photo-room/core";
import { t } from "../lib/i18n";
import {
  normalizeObject,
  parseViewerMesh,
  viewerObjectBounds,
  visibleObjectSphere,
  type ModelView,
  type ViewerGeometry,
  type ViewerObject,
} from "@volum/viewer/core";
export { normalizeObject, parseViewerMesh, viewerObjectBounds } from "@volum/viewer/core";
export type { ModelView, ViewerGeometry, ViewerObject } from "@volum/viewer/core";
import { cachedViewerPayload } from "../lib/viewerMeshCache";
import { createBambuMaterial } from "@volum/viewer/materials";

const VIEW_POSITIONS: Record<ModelView, [number, number, number]> = {
  iso: [3.2, 2.35, 4.1],
  front: [0, 0, 5],
  side: [5, 0, 0],
  top: [0, 5, 0.001]
};

function Controls({ resetSignal, view, focusBounds, focusSignal, enabled = true, fitObject, fitToFrame = false }: { resetSignal: number; view: ModelView; focusBounds?: THREE.Box3; focusSignal: number; enabled?: boolean; fitObject?: THREE.Object3D; fitToFrame?: boolean }) {
  const { camera, gl, invalidate, size } = useThree();
  const controls = useMemo(() => new OrbitControls(camera, gl.domElement), [camera, gl.domElement]);
  useEffect(() => {
    const onChange = () => invalidate();
    controls.enableDamping = true;
    controls.dampingFactor = .08;
    controls.minDistance = 0.12;
    controls.maxDistance = 12;
    controls.addEventListener("change", onChange);
    return () => { controls.removeEventListener("change", onChange); controls.dispose(); };
  }, [controls, invalidate]);
  useEffect(() => {
    controls.maxDistance = fitToFrame ? 50 : 12;
  }, [controls, fitToFrame]);
  useEffect(() => {
    controls.enabled = enabled;
    return () => { controls.enabled = true; };
  }, [controls, enabled]);
  useEffect(() => {
    const [x, y, z] = VIEW_POSITIONS[view];
    camera.up.set(0, view === "top" ? 0 : 1, view === "top" ? -1 : 0);
    camera.position.set(x, y, z);
    controls.target.set(0, 0, 0);
    controls.update();
    invalidate();
  }, [camera, controls, invalidate, resetSignal, view]);
  useEffect(() => {
    if (!fitToFrame || !fitObject || !(camera instanceof THREE.PerspectiveCamera)) return;
    const sphere = visibleObjectSphere(fitObject);
    if (!sphere || !Number.isFinite(sphere.radius) || sphere.radius <= 0) return;
    const direction = camera.position.clone().sub(controls.target).normalize();
    if (!direction.lengthSq()) direction.set(0.6, 0.45, 0.7).normalize();
    const distance = photoRoomCameraFitDistance(
      sphere.radius,
      camera.fov,
      size.width / Math.max(1, size.height),
    );
    controls.target.copy(sphere.center);
    camera.position.copy(sphere.center).addScaledVector(direction, distance);
    camera.near = Math.max(0.01, distance - sphere.radius * 2);
    camera.far = Math.max(camera.near + 1, distance + sphere.radius * 4);
    camera.updateProjectionMatrix();
    controls.update();
    invalidate();
  }, [camera, controls, fitObject, fitToFrame, invalidate, resetSignal, size.height, size.width]);
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

function disposeObject(object?: THREE.Object3D) {
  object?.traverse((child) => {
    if (child instanceof THREE.Mesh) {
      child.geometry.disposeBoundsTree?.();
      child.geometry.dispose();
      if (Array.isArray(child.material)) child.material.forEach((material) => material.dispose());
      else if (child.material instanceof THREE.Material) child.material.dispose();
    }
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

function LoadedObject({ assetId, extension, plateIndex, revision, selectedObjectId, isolatedObjectId, hideBuildPlate = false, onObjectSelect, onLoaded, onObjects, onError, onReady }: { assetId: string; extension: string; plateIndex?: number; revision?: string; selectedObjectId?: number; isolatedObjectId?: number; hideBuildPlate?: boolean; onObjectSelect?: (objectId?: number) => void; onLoaded: (object?: THREE.Object3D) => void; onObjects: (objects: ViewerObject[]) => void; onError: (message: string) => void; onReady: (geometry: ViewerGeometry) => void }) {
  const [object, setObject] = useState<THREE.Object3D>();
  const invalidate = useThree((state) => state.invalidate);
  useEffect(() => {
    let disposed = false;
    let loaded: THREE.Object3D | undefined;
    setObject(undefined);
    onLoaded(undefined);
    onObjects([]);
    cachedViewerPayload(assetId, plateIndex, revision).then((buffer) => {
      if (disposed || buffer.byteLength === 0) return;
      const parsed = parseViewerMesh(buffer, t);
      const sourceBounds = parsed.userData.volumSourceBounds;
      const size = (sourceBounds instanceof THREE.Box3 ? sourceBounds : new THREE.Box3().setFromObject(parsed))
        .getSize(new THREE.Vector3());
      let triangleCount = 0;
      parsed.traverse((child) => {
        if (child instanceof THREE.Mesh) triangleCount += (child.geometry.index?.count ?? child.geometry.getAttribute("position").count) / 3;
      });
      loaded = normalizeObject(parsed, hideBuildPlate, t);
      if (disposed) {
        disposeObject(loaded);
        return;
      }
      setObject(loaded);
      onLoaded(loaded);
      onObjects((loaded.userData.volumObjects as ViewerObject[] | undefined) ?? []);
      onReady({ dimensionsMm: [size.x, size.y, size.z], triangleCount });
    }).catch((error: unknown) => {
      if (!disposed) onError(error instanceof Error ? error.message : String(error));
    });
    return () => {
      disposed = true;
      onLoaded(undefined);
      disposeObject(loaded);
    };
  }, [assetId, extension, hideBuildPlate, onError, onLoaded, onObjects, onReady, plateIndex, revision]);
  useEffect(() => {
    if (!object) return;
    object.traverse((child) => {
      if (!(child instanceof THREE.Mesh)) return;
      const materials = Array.isArray(child.material) ? child.material : [child.material];
      materials.forEach((material) => {
        if (material.userData.volumMaterialKind === "plate-surface" || material.userData.volumMaterialKind === "plate-detail") {
          material.visible = !hideBuildPlate;
        }
        if (!(material instanceof THREE.ShaderMaterial) || !material.uniforms.selected_object_id) return;
        material.uniforms.selected_object_id.value = selectedObjectId ?? 0;
        material.uniforms.isolated_object_id.value = isolatedObjectId ?? 0;
      });
    });
    invalidate();
  }, [hideBuildPlate, invalidate, isolatedObjectId, object, selectedObjectId]);
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

function DemoObject({ modelId }: { modelId: string }) {
  const index = Number(modelId.match(/\d+/)?.[0] ?? 1) % 4;
  if (index === 0) return <group rotation={[.1, .25, 0]}><mesh position={[0, -.52, 0]}><cylinderGeometry args={[.72, .88, .18, 6]} /><DemoMaterial /></mesh><mesh position={[0, .02, 0]}><cylinderGeometry args={[.38, .7, .95, 6]} /><DemoMaterial /></mesh><mesh position={[0, .6, 0]}><torusGeometry args={[.37, .08, 12, 6]} /><DemoMaterial /></mesh></group>;
  if (index === 1) return <group rotation={[.15, -.3, -.08]}>{[-.55, 0, .55].map((x) => <mesh key={x} position={[x, 0, 0]}><torusKnotGeometry args={[.34, .095, 80, 12, 2, 3]} /><DemoMaterial /></mesh>)}</group>;
  if (index === 2) return <mesh rotation={[0, .3, 0]}><icosahedronGeometry args={[1, 2]} /><DemoMaterial /></mesh>;
  return <group rotation={[.2, -.25, 0]}><mesh><boxGeometry args={[1.7, .22, 1.2]} /><DemoMaterial /></mesh><mesh position={[-.65, .45, 0]}><boxGeometry args={[.22, .8, 1.2]} /><DemoMaterial /></mesh><mesh position={[.65, .45, 0]}><boxGeometry args={[.22, .8, 1.2]} /><DemoMaterial /></mesh></group>;
}

function DemoMaterial() {
  const material = useMemo(() => createBambuMaterial([216, 200, 170]), []);
  useEffect(() => () => material.dispose(), [material]);
  return <primitive object={material} attach="material" />;
}

export function ModelViewer({ assetId, extension, modelId, plateIndex, revision, resetSignal = 0, view = "iso", selectedObjectId, isolatedObjectId, focusObjectId, focusSignal = 0, capture = false, modelOrientation = [0, 0, 0, 1], interactionEnabled = true, fitToFrame = false, hideBuildPlate = false, onObjectSelect, onObjects, onError, onGeometry }: { assetId?: string; extension: string; modelId: string; plateIndex?: number; revision?: string; resetSignal?: number; view?: ModelView; selectedObjectId?: number; isolatedObjectId?: number; focusObjectId?: number; focusSignal?: number; capture?: boolean; modelOrientation?: [number, number, number, number]; interactionEnabled?: boolean; fitToFrame?: boolean; hideBuildPlate?: boolean; onObjectSelect?: (objectId?: number) => void; onObjects?: (objects: ViewerObject[]) => void; onError?: (message: string) => void; onGeometry?: (geometry: ViewerGeometry) => void }) {
  const demo = modelId.startsWith("demo-");
  const [status, setStatus] = useState<"loading" | "ready" | "error">(demo || !assetId ? "ready" : "loading");
  const [loadedObject, setLoadedObject] = useState<THREE.Object3D>();
  const [fallbackObject, setFallbackObject] = useState<THREE.Group>();
  const handleFallbackObject = useCallback((object: THREE.Group | null) => setFallbackObject(object ?? undefined), []);
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
  const handleObjects = useCallback((objects: ViewerObject[]) => onObjects?.(objects), [onObjects]);
  const focusBounds = useMemo(
    () => loadedObject && focusObjectId ? viewerObjectBounds(loadedObject, focusObjectId) : undefined,
    [focusObjectId, focusSignal, loadedObject],
  );
  return (
    <>
      <Canvas flat dpr={capture ? 2.25 : [1, 1.75]} camera={{ position: VIEW_POSITIONS.iso, fov: 34 }} frameloop="demand" gl={{ antialias: true, alpha: true, preserveDrawingBuffer: capture }} onContextMenu={(event) => event.preventDefault()} onPointerMissed={() => { if (!isolatedObjectId) onObjectSelect?.(undefined); }}>
        <Suspense fallback={null}>
          <group quaternion={modelOrientation}>{demo || !assetId ? <group ref={handleFallbackObject}><DemoObject modelId={modelId} /></group> : <LoadedObject assetId={assetId} extension={extension} plateIndex={plateIndex} revision={revision} selectedObjectId={selectedObjectId} isolatedObjectId={isolatedObjectId} hideBuildPlate={hideBuildPlate} onObjectSelect={onObjectSelect} onLoaded={setLoadedObject} onObjects={handleObjects} onError={handleError} onReady={handleReady} />}</group>
        </Suspense>
        <Controls resetSignal={resetSignal} view={view} focusBounds={focusBounds} focusSignal={focusSignal} enabled={interactionEnabled} fitObject={loadedObject ?? fallbackObject} fitToFrame={fitToFrame} />
        <RaycasterSettings />
      </Canvas>
      {status === "loading" && <div className="viewer-loading"><LoaderCircle className="spin" size={18} /><span>{t("Loading 3D model…")}</span></div>}
    </>
  );
}
