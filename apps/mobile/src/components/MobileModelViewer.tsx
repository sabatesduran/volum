import { Canvas, type ThreeEvent, useFrame, useThree } from "@react-three/fiber";
import { CircleAlert, LoaderCircle } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import * as THREE from "three";
import { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";
import { photoRoomCameraFitDistance } from "@volum/photo-room/core";
import { t } from "../lib/i18n";
import {
  normalizeObject,
  parseViewerMesh,
  viewerObjectBounds,
  visibleObjectSphere,
  type ViewerObject,
} from "@volum/viewer/core";
export { normalizeObject, parseViewerMesh, viewerObjectBounds } from "@volum/viewer/core";
import { requestCachedBinary } from "../lib/mediaCache";
import type { Credential } from "../types";
import type { LocalStlObject } from "../lib/localStl";

const viewerPayloads = new Map<string, Promise<ArrayBuffer>>();
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

function Controls({ resetSignal, focusBounds, focusSignal, enabled = true, fitObject, fitToFrame = false }: { resetSignal: number; focusBounds?: THREE.Box3; focusSignal: number; enabled?: boolean; fitObject?: THREE.Object3D; fitToFrame?: boolean }) {
  const { camera, gl, invalidate, size } = useThree();
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
    controls.enabled = enabled;
    return () => { controls.enabled = true; };
  }, [controls, enabled]);
  useEffect(() => {
    controls.maxDistance = fitToFrame ? 50 : 12;
  }, [controls, fitToFrame]);
  useEffect(() => {
    camera.up.set(0, 1, 0);
    camera.position.set(3.2, 2.35, 4.1);
    controls.target.set(0, 0, 0);
    controls.update();
    invalidate();
  }, [camera, controls, invalidate, resetSignal]);
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

function LoadedModel({ credential, assetId, revision, plateIndex, offlinePayload, localPayload, selectedObjectId, isolatedObjectId, hideBuildPlate = false, onObjectSelect, onLoaded, onStatus }: { credential?: Credential; assetId?: string; revision: string; plateIndex?: number; offlinePayload?: ArrayBuffer; localPayload?: ArrayBuffer; selectedObjectId?: number; isolatedObjectId?: number; hideBuildPlate?: boolean; onObjectSelect?: (objectId?: number) => void; onLoaded: (object?: THREE.Object3D) => void; onStatus: (status: "ready" | "error", message?: string) => void }) {
  const [object, setObject] = useState<THREE.Object3D>();
  const invalidate = useThree((state) => state.invalidate);
  useEffect(() => {
    let active = true;
    let loaded: THREE.Object3D | undefined;
    setObject(undefined);
    onLoaded(undefined);
    void viewerPayload(credential, assetId, revision, plateIndex, offlinePayload, localPayload).then((buffer) => {
      loaded = normalizeObject(parseViewerMesh(buffer, t), hideBuildPlate, t);
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
  }, [assetId, credential, hideBuildPlate, localPayload, offlinePayload, onLoaded, onStatus, plateIndex, revision]);
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

export function MobileModelViewer({ credential, assetId, revision = "local", plateIndex, offlinePayload, localPayload, resetSignal, selectedObjectId, isolatedObjectId, focusObjectId, focusSignal = 0, capture = false, modelOrientation = [0, 0, 0, 1], interactionEnabled = true, fitToFrame = false, hideBuildPlate = false, onObjectSelect, onObjects, onStatus }: { credential?: Credential; assetId?: string; revision?: string; plateIndex?: number; offlinePayload?: ArrayBuffer; localPayload?: ArrayBuffer; resetSignal: number; selectedObjectId?: number; isolatedObjectId?: number; focusObjectId?: number; focusSignal?: number; capture?: boolean; modelOrientation?: [number, number, number, number]; interactionEnabled?: boolean; fitToFrame?: boolean; hideBuildPlate?: boolean; onObjectSelect?: (objectId?: number) => void; onObjects?: (objects: ViewerObject[]) => void; onStatus: (status: "ready" | "error", message?: string) => void }) {
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
    <Canvas flat dpr={capture ? 3 : [1, 1.5]} camera={{ position: [3.2, 2.35, 4.1], fov: 34 }} frameloop="demand" gl={{ antialias: true, alpha: true, preserveDrawingBuffer: capture, powerPreference: "high-performance" }} onPointerMissed={() => { if (!isolatedObjectId) onObjectSelect?.(undefined); }}>
      <group quaternion={modelOrientation}><LoadedModel credential={credential} assetId={assetId} revision={revision} plateIndex={plateIndex} offlinePayload={offlinePayload} localPayload={localPayload} selectedObjectId={selectedObjectId} isolatedObjectId={isolatedObjectId} hideBuildPlate={hideBuildPlate} onObjectSelect={onObjectSelect} onLoaded={setLoadedObject} onStatus={handleStatus} /></group>
      <Controls resetSignal={resetSignal} focusBounds={focusBounds} focusSignal={focusSignal} enabled={interactionEnabled} fitObject={loadedObject} fitToFrame={fitToFrame} />
      <RaycasterSettings />
    </Canvas>
    {status === "loading" && <div className="viewer-state" role="status"><LoaderCircle className="spin" /><span>{t("Preparing 3D model…")}</span></div>}
    {status === "error" && <div className="viewer-state viewer-state--error" role="alert"><CircleAlert /><span>{t(message || "This model could not be displayed.")}</span></div>}
  </div>;
}
