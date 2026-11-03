export type PhotoRoomFormatId = "square" | "portrait" | "landscape" | "story";

export interface PhotoRoomFormat {
  id: PhotoRoomFormatId;
  label: string;
  detail: string;
  width: number;
  height: number;
}

export const PHOTO_ROOM_FORMATS: readonly PhotoRoomFormat[] = [
  { id: "square", label: "Square", detail: "1:1 · 1080 × 1080", width: 1080, height: 1080 },
  { id: "portrait", label: "Portrait", detail: "4:5 · 1080 × 1350", width: 1080, height: 1350 },
  { id: "landscape", label: "Landscape", detail: "1.91:1 · 1080 × 566", width: 1080, height: 566 },
  { id: "story", label: "Reel / Story", detail: "9:16 · 1080 × 1920", width: 1080, height: 1920 },
] as const;

export type PhotoRoomBackground =
  | { type: "solid"; color: string }
  | { type: "gradient"; from: string; to: string; angle: number }
  | { type: "photo"; url: string; name: string };

export type PhotoRoomTextStyle = "clean" | "glow" | "outline";
export type PhotoRoomAnimationType = "spin" | "flip" | "roll" | "tumble";
export type PhotoRoomQuaternion = [number, number, number, number];
export interface PhotoRoomPoint { x: number; y: number }

export interface PhotoRoomTextLayer {
  id: string;
  type: "text";
  text: string;
  x: number;
  y: number;
  scale: number;
  rotation: number;
  fontSize: number;
  fontFamily: "sans" | "serif" | "mono";
  fontWeight: 500 | 700 | 900;
  color: string;
  style: PhotoRoomTextStyle;
  maxWidth: number;
}

export interface PhotoRoomDocument {
  version: 1;
  format: PhotoRoomFormatId;
  background: PhotoRoomBackground;
  model: {
    x: number;
    y: number;
    scale: number;
    rotation: number;
    yaw: number;
    pitch: number;
  };
  texts: PhotoRoomTextLayer[];
  animation: {
    type: PhotoRoomAnimationType;
    duration: 6 | 10 | 15;
    turns: 1 | 2;
    direction: 1 | -1;
  };
}

export function createPhotoRoomDocument(): PhotoRoomDocument {
  return {
    version: 1,
    format: "square",
    background: { type: "gradient", from: "#f6dfbd", to: "#ff6b46", angle: 145 },
    model: { x: 0.5, y: 0.52, scale: 0.92, rotation: 0, yaw: 0, pitch: 0 },
    texts: [],
    animation: { type: "spin", duration: 6, turns: 1, direction: 1 },
  };
}

export function photoRoomFormat(id: PhotoRoomFormatId): PhotoRoomFormat {
  return PHOTO_ROOM_FORMATS.find((format) => format.id === id) ?? PHOTO_ROOM_FORMATS[0];
}

export function clampPhotoRoomDocument(document: PhotoRoomDocument): PhotoRoomDocument {
  const clamp = (value: number, minimum: number, maximum: number) => Math.min(maximum, Math.max(minimum, value));
  const wrapRadians = (value: number) => {
    const turn = Math.PI * 2;
    return ((value + Math.PI) % turn + turn) % turn - Math.PI;
  };
  return {
    ...document,
    model: {
      x: clamp(document.model.x, 0, 1),
      y: clamp(document.model.y, 0, 1),
      scale: clamp(document.model.scale, 0.35, 2.5),
      rotation: document.model.rotation,
      yaw: wrapRadians(document.model.yaw),
      pitch: clamp(document.model.pitch, -Math.PI * 4 / 9, Math.PI * 4 / 9),
    },
    texts: document.texts.map((text) => ({
      ...text,
      style: (text.style as string) === "label" ? "glow" : text.style,
      x: clamp(text.x, 0.02, 0.98),
      y: clamp(text.y, 0.02, 0.98),
      scale: clamp(text.scale, 0.35, 4),
      fontSize: clamp(text.fontSize, 0.025, 0.2),
      maxWidth: clamp(text.maxWidth, 0.2, 0.94),
    })),
    animation: {
      ...document.animation,
      type: document.animation.type ?? "spin",
    },
  };
}

export type PhotoRoomRotationMode = "turntable" | "free";

export function photoRoomRotationFromDrag(
  origin: { yaw: number; pitch: number },
  delta: { x: number; y: number },
  stage: { width: number; height: number },
  mode: PhotoRoomRotationMode,
) {
  const width = Math.max(1, stage.width);
  const height = Math.max(1, stage.height);
  return {
    yaw: origin.yaw + delta.x / width * Math.PI,
    pitch: mode === "free" ? origin.pitch + delta.y / height * Math.PI * 0.75 : origin.pitch,
  };
}

export function photoRoomCameraFitDistance(
  radius: number,
  verticalFovDegrees: number,
  aspect: number,
  frameMargin = 0.05,
) {
  const safeRadius = Math.max(0.0001, radius);
  const safeAspect = Math.max(0.01, aspect);
  const verticalHalfFov = Math.max(0.01, Math.min(Math.PI * 0.49, verticalFovDegrees * Math.PI / 360));
  const horizontalHalfFov = Math.atan(Math.tan(verticalHalfFov) * safeAspect);
  const limitingHalfFov = Math.min(verticalHalfFov, horizontalHalfFov);
  const usableFrame = Math.max(0.1, 1 - Math.max(0, Math.min(0.4, frameMargin)) * 2);
  return safeRadius / Math.sin(limitingHalfFov) / usableFrame;
}

function quaternionFromAxisAngle(axis: readonly [number, number, number], angle: number): PhotoRoomQuaternion {
  const length = Math.hypot(axis[0], axis[1], axis[2]) || 1;
  const halfAngle = angle / 2;
  const scale = Math.sin(halfAngle) / length;
  return [axis[0] * scale, axis[1] * scale, axis[2] * scale, Math.cos(halfAngle)];
}

function multiplyQuaternions(left: PhotoRoomQuaternion, right: PhotoRoomQuaternion): PhotoRoomQuaternion {
  const [lx, ly, lz, lw] = left;
  const [rx, ry, rz, rw] = right;
  return [
    lw * rx + lx * rw + ly * rz - lz * ry,
    lw * ry - lx * rz + ly * rw + lz * rx,
    lw * rz + lx * ry - ly * rx + lz * rw,
    lw * rw - lx * rx - ly * ry - lz * rz,
  ];
}

function normalizeQuaternion(quaternion: PhotoRoomQuaternion): PhotoRoomQuaternion {
  const length = Math.hypot(...quaternion) || 1;
  return quaternion.map((value) => value / length) as PhotoRoomQuaternion;
}

const PHOTO_ROOM_ANIMATION_AXES: Record<PhotoRoomAnimationType, readonly [number, number, number]> = {
  spin: [0, 1, 0],
  flip: [1, 0, 0],
  roll: [0, 0, 1],
  tumble: [1, 1, 0],
};

export function photoRoomModelOrientation(
  model: Pick<PhotoRoomDocument["model"], "yaw" | "pitch">,
  animation: Pick<PhotoRoomDocument["animation"], "type" | "turns" | "direction">,
  progress = 0,
): PhotoRoomQuaternion {
  const yaw = quaternionFromAxisAngle([0, 1, 0], model.yaw);
  const pitch = quaternionFromAxisAngle([1, 0, 0], model.pitch);
  const startingPose = multiplyQuaternions(yaw, pitch);
  const animatedRotation = quaternionFromAxisAngle(
    PHOTO_ROOM_ANIMATION_AXES[animation.type],
    progress * Math.PI * 2 * animation.turns * animation.direction,
  );
  return normalizeQuaternion(multiplyQuaternions(animatedRotation, startingPose));
}

export function photoRoomTextTransformFromGesture(
  origin: Pick<PhotoRoomTextLayer, "x" | "y" | "scale" | "rotation">,
  originPoints: readonly PhotoRoomPoint[],
  currentPoints: readonly PhotoRoomPoint[],
  stage: { width: number; height: number },
) {
  const width = Math.max(1, stage.width);
  const height = Math.max(1, stage.height);
  if (originPoints.length >= 2 && currentPoints.length >= 2) {
    const originCenter = {
      x: (originPoints[0].x + originPoints[1].x) / 2,
      y: (originPoints[0].y + originPoints[1].y) / 2,
    };
    const currentCenter = {
      x: (currentPoints[0].x + currentPoints[1].x) / 2,
      y: (currentPoints[0].y + currentPoints[1].y) / 2,
    };
    const originDistance = Math.max(1, Math.hypot(
      originPoints[1].x - originPoints[0].x,
      originPoints[1].y - originPoints[0].y,
    ));
    const currentDistance = Math.hypot(
      currentPoints[1].x - currentPoints[0].x,
      currentPoints[1].y - currentPoints[0].y,
    );
    return {
      x: origin.x + (currentCenter.x - originCenter.x) / width,
      y: origin.y + (currentCenter.y - originCenter.y) / height,
      scale: origin.scale * currentDistance / originDistance,
      rotation: origin.rotation,
    };
  }
  if (originPoints.length && currentPoints.length) {
    return {
      x: origin.x + (currentPoints[0].x - originPoints[0].x) / width,
      y: origin.y + (currentPoints[0].y - originPoints[0].y) / height,
      scale: origin.scale,
      rotation: origin.rotation,
    };
  }
  return { ...origin };
}

export function photoRoomTextRotationFromPointer(
  originRotation: number,
  center: PhotoRoomPoint,
  originPointer: PhotoRoomPoint,
  currentPointer: PhotoRoomPoint,
  snapIncrement = 0,
) {
  const originAngle = Math.atan2(originPointer.y - center.y, originPointer.x - center.x);
  const currentAngle = Math.atan2(currentPointer.y - center.y, currentPointer.x - center.x);
  const fullTurn = Math.PI * 2;
  const delta = ((currentAngle - originAngle + Math.PI) % fullTurn + fullTurn) % fullTurn - Math.PI;
  const rotation = originRotation + delta * 180 / Math.PI;
  return snapIncrement > 0 ? Math.round(rotation / snapIncrement) * snapIncrement : rotation;
}

export function photoRoomTextScaleFromPointer(
  originScale: number,
  center: PhotoRoomPoint,
  originPointer: PhotoRoomPoint,
  currentPointer: PhotoRoomPoint,
) {
  const originDistance = Math.max(1, Math.hypot(originPointer.x - center.x, originPointer.y - center.y));
  const currentDistance = Math.hypot(currentPointer.x - center.x, currentPointer.y - center.y);
  return originScale * currentDistance / originDistance;
}

export function createTextLayer(text = "Your text"): PhotoRoomTextLayer {
  return {
    id: crypto.randomUUID(),
    type: "text",
    text,
    x: 0.5,
    y: 0.16,
    scale: 1,
    rotation: 0,
    fontSize: 0.07,
    fontFamily: "sans",
    fontWeight: 900,
    color: "#ffffff",
    style: "clean",
    maxWidth: 0.82,
  };
}

export function safePhotoRoomFilename(value: string) {
  return value.trim().replace(/[\\/:*?"<>|]+/g, "-").replace(/\s+/g, " ").slice(0, 100) || "model";
}
