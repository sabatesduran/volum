import { useEffect, useRef, useState } from "react";
import type { LocalStlObject } from "../lib/localStl";

const MAX_THUMBNAIL_TRIANGLES = 2_000;
const THUMBNAIL_SIZE = 72;

type Point3 = readonly [number, number, number];

const normalize = ([x, y, z]: Point3): Point3 => {
  const length = Math.hypot(x, y, z) || 1;
  return [x / length, y / length, z / length];
};
const dot = (left: Point3, right: Point3) => left[0] * right[0] + left[1] * right[1] + left[2] * right[2];
const cross = (left: Point3, right: Point3): Point3 => [
  left[1] * right[2] - left[2] * right[1],
  left[2] * right[0] - left[0] * right[2],
  left[0] * right[1] - left[1] * right[0],
];
const subtract = (left: Point3, right: Point3): Point3 => [left[0] - right[0], left[1] - right[1], left[2] - right[2]];
const cameraDirection = normalize([1.35, -1.65, 1.15]);
const cameraRight = normalize(cross(cameraDirection, [0, 0, 1]));
const cameraUp = normalize(cross(cameraRight, cameraDirection));
const lightDirection = normalize([-0.45, -0.7, 1]);

interface ViewerLayout {
  view: DataView;
  vertexCount: number;
  vertexOffset: number;
  indexOffset: number;
  colorOffset: number;
  objectIdOffset: number;
  triangleCount: number;
}

function viewerLayout(payload: ArrayBuffer): ViewerLayout | undefined {
  if (payload.byteLength < 40 || String.fromCharCode(...new Uint8Array(payload, 0, 4)) !== "VLM5") return undefined;
  const view = new DataView(payload);
  const vertexCount = view.getUint32(4, true);
  const indexCount = view.getUint32(8, true);
  const objectCount = view.getUint32(36, true);
  if (indexCount % 3 !== 0 || objectCount > 4_096) return undefined;
  const triangleCount = indexCount / 3;
  const vertexOffset = 40 + objectCount * 32;
  const indexOffset = vertexOffset + vertexCount * 12;
  const colorOffset = indexOffset + indexCount * 4;
  const kindOffset = colorOffset + triangleCount * 3;
  const objectIdOffset = kindOffset + triangleCount;
  const payloadEnd = objectIdOffset + triangleCount * 4;
  if (![vertexOffset, indexOffset, colorOffset, kindOffset, objectIdOffset, payloadEnd].every(Number.isSafeInteger)
    || payloadEnd > payload.byteLength) return undefined;
  return { view, vertexCount, vertexOffset, indexOffset, colorOffset, objectIdOffset, triangleCount };
}

export function buildObjectPreviewSamples(payload: ArrayBuffer, objects: LocalStlObject[]) {
  const layout = viewerLayout(payload);
  const samples = new Map<number, number[]>();
  if (!layout) return samples;
  const steps = new Map(objects.map((object) => [object.id, Math.max(1, Math.ceil(object.triangleCount / MAX_THUMBNAIL_TRIANGLES))]));
  const seen = new Map<number, number>();
  for (let triangle = 0; triangle < layout.triangleCount; triangle += 1) {
    const objectId = layout.view.getUint32(layout.objectIdOffset + triangle * 4, true);
    const step = steps.get(objectId);
    if (!step) continue;
    const occurrence = seen.get(objectId) ?? 0;
    seen.set(objectId, occurrence + 1);
    if (occurrence % step !== 0) continue;
    const objectSamples = samples.get(objectId) ?? [];
    if (objectSamples.length < MAX_THUMBNAIL_TRIANGLES) objectSamples.push(triangle);
    samples.set(objectId, objectSamples);
  }
  return samples;
}

export function ObjectPreviewThumbnail({ payload, object, samples }: { payload: ArrayBuffer; object: LocalStlObject; samples?: number[] }) {
  const containerRef = useRef<HTMLSpanElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [shouldRender, setShouldRender] = useState(false);
  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;
    if (!("IntersectionObserver" in window)) {
      setShouldRender(true);
      return;
    }
    const observer = new IntersectionObserver((entries) => {
      if (!entries.some((entry) => entry.isIntersecting)) return;
      setShouldRender(true);
      observer.disconnect();
    }, { rootMargin: "160px" });
    observer.observe(container);
    return () => observer.disconnect();
  }, []);
  useEffect(() => {
    const canvas = canvasRef.current;
    const layout = viewerLayout(payload);
    if (!shouldRender || !canvas || !layout || !samples?.length) return;
    const size = THUMBNAIL_SIZE;
    const ratio = Math.min(window.devicePixelRatio || 1, 2);
    canvas.width = size * ratio;
    canvas.height = size * ratio;
    const context = canvas.getContext("2d");
    if (!context) return;
    context.scale(ratio, ratio);
    context.clearRect(0, 0, size, size);

    const [minX, minY, minZ, maxX, maxY, maxZ] = object.bounds;
    const center: Point3 = [(minX + maxX) / 2, (minY + maxY) / 2, (minZ + maxZ) / 2];
    const project = (x: number, y: number, z: number) => {
      const point: Point3 = [x - center[0], y - center[1], z - center[2]];
      return [dot(point, cameraRight), -dot(point, cameraUp), dot(point, cameraDirection)] as const;
    };
    const corners = [minX, maxX].flatMap((x) => [minY, maxY].flatMap((y) => [minZ, maxZ].map((z) => project(x, y, z))));
    const projectedWidth = Math.max(...corners.map((point) => point[0])) - Math.min(...corners.map((point) => point[0]));
    const projectedHeight = Math.max(...corners.map((point) => point[1])) - Math.min(...corners.map((point) => point[1]));
    const scale = 57 / Math.max(projectedWidth, projectedHeight, 0.000_001);
    const triangles = samples.flatMap((triangle) => {
      if (triangle < 0 || triangle >= layout.triangleCount) return [];
      const worldPoints: Point3[] = [];
      for (let corner = 0; corner < 3; corner += 1) {
        const index = layout.view.getUint32(layout.indexOffset + (triangle * 3 + corner) * 4, true);
        if (index >= layout.vertexCount) return [];
        const offset = layout.vertexOffset + index * 12;
        const point = [layout.view.getFloat32(offset, true), layout.view.getFloat32(offset + 4, true), layout.view.getFloat32(offset + 8, true)] as Point3;
        if (!point.every(Number.isFinite)) return [];
        worldPoints.push(point);
      }
      const points = worldPoints.map((point) => project(...point));
      const normal = normalize(cross(subtract(worldPoints[1], worldPoints[0]), subtract(worldPoints[2], worldPoints[0])));
      const colorOffset = layout.colorOffset + triangle * 3;
      return [{
        points,
        depth: points.reduce((sum, point) => sum + point[2], 0) / 3,
        color: [layout.view.getUint8(colorOffset), layout.view.getUint8(colorOffset + 1), layout.view.getUint8(colorOffset + 2)],
        light: 0.48 + Math.abs(dot(normal, lightDirection)) * 0.52,
      }];
    }).sort((left, right) => left.depth - right.depth);
    if (!triangles.length) return;

    const projectedBottom = Math.max(...corners.map((point) => point[1])) * scale + size / 2;
    const shadowWidth = Math.min(48, Math.max(20, projectedWidth * scale * 0.72));
    const shadow = context.createRadialGradient(size / 2, projectedBottom + 1, 1, size / 2, projectedBottom + 1, shadowWidth / 2);
    shadow.addColorStop(0, "rgb(18 22 28 / .2)");
    shadow.addColorStop(1, "rgb(18 22 28 / 0)");
    context.save();
    context.scale(1, 0.24);
    context.fillStyle = shadow;
    context.fillRect(size / 2 - shadowWidth / 2, (projectedBottom - 4) / 0.24, shadowWidth, 13 / 0.24);
    context.restore();

    context.lineJoin = "round";
    for (const triangle of triangles) {
      context.beginPath();
      triangle.points.forEach((point, index) => {
        const x = size / 2 + point[0] * scale;
        const y = size / 2 + point[1] * scale;
        if (index === 0) context.moveTo(x, y); else context.lineTo(x, y);
      });
      context.closePath();
      const depthShade = 0.92 + (triangle.depth - triangles[0].depth) / Math.max(0.000_001, triangles.at(-1)!.depth - triangles[0].depth) * 0.08;
      const shaded = triangle.color.map((channel) => Math.round(Math.min(255, Math.max(20, channel * triangle.light * depthShade))));
      context.fillStyle = `rgb(${shaded.join(" ")})`;
      context.fill();
      context.strokeStyle = "rgb(17 22 29 / .16)";
      context.lineWidth = 0.35;
      context.stroke();
    }
  }, [object, payload, samples, shouldRender]);
  return <span ref={containerRef} className={`object-preview-thumbnail${samples?.length && shouldRender ? "" : " is-loading"}`}>
    {shouldRender && <canvas ref={canvasRef} width={THUMBNAIL_SIZE} height={THUMBNAIL_SIZE} aria-hidden="true" />}
  </span>;
}
