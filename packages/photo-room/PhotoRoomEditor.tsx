import {
  ArrowLeft, Check, ChevronDown, Download, Image as ImageIcon, LoaderCircle, Move,
  Maximize2, Play, Plus, Redo2, Rotate3D, RotateCw, Share2, Sparkles, Square, Trash2, Type, Undo2, X,
} from "lucide-react";
import {
  type CSSProperties, type KeyboardEvent as ReactKeyboardEvent, type PointerEvent as ReactPointerEvent, type ReactNode,
  useCallback, useEffect, useMemo, useRef, useState,
} from "react";
import {
  PHOTO_ROOM_FORMATS, clampPhotoRoomDocument, createPhotoRoomDocument, createTextLayer,
  photoRoomFormat, photoRoomModelOrientation, photoRoomRotationFromDrag,
  photoRoomTextRotationFromPointer, photoRoomTextScaleFromPointer,
  photoRoomTextTransformFromGesture, safePhotoRoomFilename, type PhotoRoomAnimationType,
  type PhotoRoomDocument, type PhotoRoomPoint, type PhotoRoomQuaternion,
  type PhotoRoomRotationMode, type PhotoRoomTextLayer,
} from "./core";
import "./photo-room.css";

export interface PhotoRoomModelOptions {
  capture: boolean;
  orientation: PhotoRoomQuaternion;
  interactionEnabled: boolean;
  resetSignal: number;
}

export interface PhotoRoomPreparedMedia {
  file: File;
  kind: "photo" | "video";
  poster?: File;
}

interface PhotoRoomEditorProps {
  title: string;
  mobile?: boolean;
  canSaveToProject?: boolean;
  renderModel: (options: PhotoRoomModelOptions) => ReactNode;
  onClose: () => void;
  onDeliver: (media: PhotoRoomPreparedMedia) => Promise<void>;
  onSaveToProject?: (file: File, useAsCover: boolean) => Promise<void>;
  translate?: (value: string, parameters?: Record<string, string | number>) => string;
}

type Tool = "format" | "background" | "model" | "text" | "animate" | "export";
type GestureTarget = "model" | string;
type ModelMode = "arrange" | PhotoRoomRotationMode;

interface GestureState {
  target: GestureTarget;
  before: PhotoRoomDocument;
  pointers: Map<number, { x: number; y: number }>;
  originPointers: Map<number, { x: number; y: number }>;
  origin: { x: number; y: number; scale: number; rotation: number };
}

interface RotationGestureState {
  mode: PhotoRoomRotationMode;
  before: PhotoRoomDocument;
  pointers: Map<number, { x: number; y: number }>;
  originPointers: Map<number, { x: number; y: number }>;
  origin: { yaw: number; pitch: number; scale: number };
}

interface TextHandleGestureState {
  mode: "rotate" | "resize";
  textId: string;
  pointerId: number;
  before: PhotoRoomDocument;
  center: PhotoRoomPoint;
  originPointer: PhotoRoomPoint;
  originRotation: number;
  originScale: number;
}

const ANIMATION_OPTIONS: Array<{ type: PhotoRoomAnimationType; label: string; detail: string }> = [
  { type: "spin", label: "Spin", detail: "Vertical axis" },
  { type: "flip", label: "Flip", detail: "Horizontal axis" },
  { type: "roll", label: "Roll", detail: "Depth axis" },
  { type: "tumble", label: "Tumble", detail: "Diagonal axis" },
];

const BACKGROUND_PRESETS = [
  { from: "#f6dfbd", to: "#ff6b46", angle: 145 },
  { from: "#d9f0ff", to: "#4f67ff", angle: 150 },
  { from: "#efe7ff", to: "#8d5cff", angle: 135 },
  { from: "#d9ffe8", to: "#1f9873", angle: 150 },
  { from: "#2c2b34", to: "#050506", angle: 145 },
] as const;

const SOLID_PRESETS = ["#fffaf0", "#ffd34e", "#ff6446", "#7b61ff", "#35a77b", "#202024"];

function cloneDocument(document: PhotoRoomDocument): PhotoRoomDocument {
  return JSON.parse(JSON.stringify(document)) as PhotoRoomDocument;
}

function nextPaint() {
  return new Promise<void>((resolve) => requestAnimationFrame(() => requestAnimationFrame(() => resolve())));
}

function canvasBlob(canvas: HTMLCanvasElement, type: string, quality?: number) {
  return new Promise<Blob>((resolve, reject) => canvas.toBlob((blob) => blob ? resolve(blob) : reject(new Error("The image could not be encoded.")), type, quality));
}

const imageCache = new Map<string, Promise<HTMLImageElement>>();
function loadImage(url: string) {
  const cached = imageCache.get(url);
  if (cached) return cached;
  const promise = new Promise<HTMLImageElement>((resolve, reject) => {
    const image = new Image();
    image.onload = () => resolve(image);
    image.onerror = () => reject(new Error("The background photo could not be read."));
    image.src = url;
  });
  imageCache.set(url, promise);
  return promise;
}

function fontFamily(layer: PhotoRoomTextLayer) {
  if (layer.fontFamily === "serif") return 'Georgia, "Times New Roman", serif';
  if (layer.fontFamily === "mono") return 'ui-monospace, "SFMono-Regular", Consolas, monospace';
  return 'Inter, ui-sans-serif, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif';
}

function wrapText(context: CanvasRenderingContext2D, text: string, maxWidth: number) {
  const output: string[] = [];
  for (const paragraph of text.split("\n")) {
    const words = paragraph.split(/\s+/).filter(Boolean);
    if (!words.length) { output.push(""); continue; }
    let line = words[0];
    for (const word of words.slice(1)) {
      const candidate = `${line} ${word}`;
      if (context.measureText(candidate).width <= maxWidth) line = candidate;
      else { output.push(line); line = word; }
    }
    output.push(line);
  }
  return output;
}

async function drawComposition(
  canvas: HTMLCanvasElement,
  document: PhotoRoomDocument,
  modelCanvas: HTMLCanvasElement | null,
) {
  const format = photoRoomFormat(document.format);
  if (canvas.width !== format.width || canvas.height !== format.height) {
    canvas.width = format.width;
    canvas.height = format.height;
  }
  const context = canvas.getContext("2d", { alpha: false });
  if (!context) throw new Error("Canvas rendering is unavailable on this device.");
  context.clearRect(0, 0, format.width, format.height);
  if (document.background.type === "solid") {
    context.fillStyle = document.background.color;
    context.fillRect(0, 0, format.width, format.height);
  } else if (document.background.type === "gradient") {
    const radians = document.background.angle * Math.PI / 180;
    const centerX = format.width / 2;
    const centerY = format.height / 2;
    const radius = Math.abs(format.width * Math.cos(radians)) + Math.abs(format.height * Math.sin(radians));
    const gradient = context.createLinearGradient(
      centerX - Math.cos(radians) * radius / 2,
      centerY - Math.sin(radians) * radius / 2,
      centerX + Math.cos(radians) * radius / 2,
      centerY + Math.sin(radians) * radius / 2,
    );
    gradient.addColorStop(0, document.background.from);
    gradient.addColorStop(1, document.background.to);
    context.fillStyle = gradient;
    context.fillRect(0, 0, format.width, format.height);
  } else {
    const image = await loadImage(document.background.url);
    const scale = Math.max(format.width / image.naturalWidth, format.height / image.naturalHeight);
    const width = image.naturalWidth * scale;
    const height = image.naturalHeight * scale;
    context.drawImage(image, (format.width - width) / 2, (format.height - height) / 2, width, height);
  }

  if (modelCanvas && modelCanvas.width > 0 && modelCanvas.height > 0) {
    context.save();
    context.translate(document.model.x * format.width, document.model.y * format.height);
    context.rotate(document.model.rotation * Math.PI / 180);
    context.scale(document.model.scale, document.model.scale);
    context.drawImage(modelCanvas, -format.width / 2, -format.height / 2, format.width, format.height);
    context.restore();
  }

  const shortSide = Math.min(format.width, format.height);
  for (const layer of document.texts) {
    const fontSize = layer.fontSize * shortSide;
    context.save();
    context.translate(layer.x * format.width, layer.y * format.height);
    context.rotate(layer.rotation * Math.PI / 180);
    context.scale(layer.scale, layer.scale);
    context.font = `${layer.fontWeight} ${fontSize}px ${fontFamily(layer)}`;
    context.textAlign = "center";
    context.textBaseline = "middle";
    context.lineJoin = "round";
    const lines = wrapText(context, layer.text || " ", layer.maxWidth * format.width / layer.scale);
    const lineHeight = fontSize * 1.08;
    const totalHeight = lines.length * lineHeight;
    lines.forEach((line, index) => {
      const y = -totalHeight / 2 + lineHeight * (index + 0.5);
      if (layer.style === "outline") {
        context.strokeStyle = "rgba(0, 0, 0, .82)";
        context.lineWidth = Math.max(4, fontSize * 0.11);
        context.strokeText(line, 0, y);
      } else if (layer.style === "glow") {
        context.shadowColor = "rgba(0, 0, 0, .78)";
        context.shadowBlur = fontSize * 0.28;
        context.shadowOffsetY = fontSize * 0.04;
      } else if (layer.style === "clean") {
        context.shadowColor = "rgba(0, 0, 0, .32)";
        context.shadowBlur = fontSize * 0.12;
        context.shadowOffsetY = fontSize * 0.05;
      }
      context.fillStyle = layer.color;
      context.fillText(line, 0, y);
      context.shadowColor = "transparent";
    });
    context.restore();
  }
}

async function encodeMp4(
  canvas: HTMLCanvasElement,
  document: PhotoRoomDocument,
  renderFrame: (frame: number, total: number) => Promise<void>,
  onProgress: (progress: number) => void,
  signal: AbortSignal,
) {
  const { BufferTarget, CanvasSource, Mp4OutputFormat, Output, Quality, getFirstEncodableVideoCodec } = await import("mediabunny");
  const format = new Mp4OutputFormat({ fastStart: "in-memory" });
  const fps = 30;
  const frameCount = document.animation.duration * fps;
  const quality = new Quality({ bitrate: document.format === "story" ? 10_000_000 : 8_000_000 });
  const codec = await getFirstEncodableVideoCodec(["avc"], { width: canvas.width, height: canvas.height, frameRate: fps, quality });
  if (!codec) {
    const mimeType = ["video/mp4;codecs=avc1.42E01E", "video/mp4"]
      .find((candidate) => typeof MediaRecorder !== "undefined" && MediaRecorder.isTypeSupported(candidate));
    if (!mimeType || typeof canvas.captureStream !== "function") {
      throw new Error("This device cannot encode H.264 video. Update the operating system or export a photo instead.");
    }
    const stream = canvas.captureStream(fps);
    const chunks: Blob[] = [];
    const recorder = new MediaRecorder(stream, { mimeType, videoBitsPerSecond: document.format === "story" ? 10_000_000 : 8_000_000 });
    const completed = new Promise<void>((resolve, reject) => {
      recorder.ondataavailable = (event) => { if (event.data.size) chunks.push(event.data); };
      recorder.onerror = (event) => reject(new Error((event as ErrorEvent).message || "The MP4 recorder failed."));
      recorder.onstop = () => resolve();
    });
    const abort = () => { if (recorder.state !== "inactive") recorder.stop(); };
    signal.addEventListener("abort", abort, { once: true });
    try {
      recorder.start(1_000);
      const started = performance.now();
      for (let frame = 0; frame < frameCount; frame += 1) {
        if (signal.aborted) throw new DOMException("Video export canceled", "AbortError");
        await renderFrame(frame, frameCount);
        const delay = started + (frame + 1) * 1_000 / fps - performance.now();
        if (delay > 0) await new Promise((resolve) => window.setTimeout(resolve, delay));
        if (frame % 3 === 0 || frame === frameCount - 1) onProgress((frame + 1) / frameCount);
      }
      recorder.stop();
      await completed;
      if (!chunks.length) throw new Error("The MP4 recorder returned an empty file.");
      return new Blob(chunks, { type: "video/mp4" });
    } finally {
      signal.removeEventListener("abort", abort);
      stream.getTracks().forEach((track) => track.stop());
    }
  }
  const target = new BufferTarget();
  const output = new Output({ format, target });
  const source = new CanvasSource(canvas, { codec, quality });
  output.addVideoTrack(source, { frameRate: fps });
  await output.start();
  try {
    for (let frame = 0; frame < frameCount; frame += 1) {
      if (signal.aborted) throw new DOMException("Video export canceled", "AbortError");
      await renderFrame(frame, frameCount);
      await source.add(frame / fps, 1 / fps, { keyFrame: frame % (fps * 2) === 0 });
      if (frame % 3 === 0 || frame === frameCount - 1) onProgress((frame + 1) / frameCount);
    }
    await output.finalize();
  } catch (error) {
    if (output.state === "started") await output.cancel().catch(() => undefined);
    throw error;
  }
  if (!target.buffer) throw new Error("The video encoder returned an empty file.");
  return new Blob([target.buffer], { type: "video/mp4" });
}

export function PhotoRoomEditor({ title, mobile = false, canSaveToProject = false, renderModel, onClose, onDeliver, onSaveToProject, translate }: PhotoRoomEditorProps) {
  const tr = useCallback((value: string, parameters?: Record<string, string | number>) => {
    if (translate) return translate(value, parameters);
    return Object.entries(parameters ?? {}).reduce(
      (output, [key, replacement]) => output.replaceAll(`{${key}}`, String(replacement)),
      value,
    );
  }, [translate]);
  const [document, setDocument] = useState(createPhotoRoomDocument);
  const documentRef = useRef(document);
  const [past, setPast] = useState<PhotoRoomDocument[]>([]);
  const [future, setFuture] = useState<PhotoRoomDocument[]>([]);
  const [tool, setTool] = useState<Tool>("background");
  const [panelExpanded, setPanelExpanded] = useState(true);
  const [selectedTextId, setSelectedTextId] = useState<string>();
  const [modelMode, setModelMode] = useState<ModelMode>("turntable");
  const [resetSignal, setResetSignal] = useState(0);
  const [stageSize, setStageSize] = useState({ width: 1, height: 1 });
  const [animationProgress, setAnimationProgress] = useState(0);
  const [orientationOverride, setOrientationOverride] = useState<PhotoRoomQuaternion>();
  const [previewing, setPreviewing] = useState(false);
  const [preparing, setPreparing] = useState<"photo" | "video">();
  const [progress, setProgress] = useState(0);
  const [prepared, setPrepared] = useState<PhotoRoomPreparedMedia>();
  const [delivering, setDelivering] = useState(false);
  const [savingProject, setSavingProject] = useState(false);
  const [useAsCover, setUseAsCover] = useState(false);
  const [message, setMessage] = useState("");
  const [error, setError] = useState("");
  const stageRef = useRef<HTMLDivElement>(null);
  const modelRootRef = useRef<HTMLDivElement>(null);
  const panelRef = useRef<HTMLElement>(null);
  const exportCanvasRef = useRef<HTMLCanvasElement>(null);
  const backgroundInputRef = useRef<HTMLInputElement>(null);
  const textInputRef = useRef<HTMLTextAreaElement>(null);
  const gestureRef = useRef<GestureState | undefined>(undefined);
  const rotationGestureRef = useRef<RotationGestureState | undefined>(undefined);
  const textHandleGestureRef = useRef<TextHandleGestureState | undefined>(undefined);
  const abortRef = useRef<AbortController | undefined>(undefined);
  const photoUrlRef = useRef<string | undefined>(undefined);
  const controlBeforeRef = useRef<PhotoRoomDocument | undefined>(undefined);

  documentRef.current = document;
  const format = photoRoomFormat(document.format);
  const selectedText = document.texts.find((text) => text.id === selectedTextId);

  useEffect(() => {
    const stage = stageRef.current;
    if (!stage) return;
    const update = () => setStageSize({ width: stage.clientWidth || 1, height: stage.clientHeight || 1 });
    update();
    const observer = new ResizeObserver(update);
    observer.observe(stage);
    return () => observer.disconnect();
  }, [format.id]);

  useEffect(() => {
    if (panelRef.current) panelRef.current.scrollTop = 0;
  }, [tool]);

  useEffect(() => () => {
    if (photoUrlRef.current) URL.revokeObjectURL(photoUrlRef.current);
    abortRef.current?.abort();
  }, []);

  useEffect(() => {
    if (!previewing) return;
    const started = performance.now();
    let frame = 0;
    const animate = (now: number) => {
      const duration = documentRef.current.animation.duration * 1000;
      setAnimationProgress(((now - started) % duration) / duration);
      frame = requestAnimationFrame(animate);
    };
    frame = requestAnimationFrame(animate);
    return () => cancelAnimationFrame(frame);
  }, [previewing, document.animation.duration]);

  const clearPrepared = useCallback(() => {
    setPrepared(undefined);
    setMessage("");
    setError("");
  }, []);

  const commit = useCallback((update: (current: PhotoRoomDocument) => PhotoRoomDocument) => {
    setDocument((current) => {
      const next = clampPhotoRoomDocument(update(current));
      setPast((history) => [...history.slice(-49), cloneDocument(current)]);
      setFuture([]);
      return next;
    });
    clearPrepared();
  }, [clearPrepared]);

  const liveUpdate = useCallback((update: (current: PhotoRoomDocument) => PhotoRoomDocument) => {
    setDocument((current) => clampPhotoRoomDocument(update(current)));
    clearPrepared();
  }, [clearPrepared]);

  const undo = () => {
    const previous = past.at(-1);
    if (!previous) return;
    setPast((history) => history.slice(0, -1));
    setFuture((history) => [cloneDocument(document), ...history].slice(0, 50));
    setDocument(cloneDocument(previous));
    clearPrepared();
  };
  const redo = () => {
    const next = future[0];
    if (!next) return;
    setFuture((history) => history.slice(1));
    setPast((history) => [...history.slice(-49), cloneDocument(document)]);
    setDocument(cloneDocument(next));
    clearPrepared();
  };
  const beginControlChange = () => { controlBeforeRef.current = cloneDocument(documentRef.current); };
  const endControlChange = () => {
    const before = controlBeforeRef.current;
    controlBeforeRef.current = undefined;
    if (!before || JSON.stringify(before) === JSON.stringify(documentRef.current)) return;
    setPast((history) => [...history.slice(-49), before]);
    setFuture([]);
  };

  const recordGesture = (before: PhotoRoomDocument) => {
    if (JSON.stringify(before) === JSON.stringify(documentRef.current)) return;
    setPast((history) => [...history.slice(-49), before]);
    setFuture([]);
  };

  const targetTransform = (target: GestureTarget, source = documentRef.current) => {
    if (target === "model") return source.model;
    return source.texts.find((text) => text.id === target);
  };

  const beginGesture = (event: ReactPointerEvent<HTMLElement>, target: GestureTarget) => {
    event.stopPropagation();
    event.currentTarget.setPointerCapture(event.pointerId);
    if (target !== "model") {
      setSelectedTextId(target);
      if (mobile) { setTool("text"); setPanelExpanded(true); }
    }
    const point = { x: event.clientX, y: event.clientY };
    const existing = gestureRef.current?.target === target ? gestureRef.current : undefined;
    const pointers = new Map(existing?.pointers ?? []);
    pointers.set(event.pointerId, point);
    const transform = targetTransform(target);
    if (!transform) return;
    gestureRef.current = {
      target,
      before: existing?.before ?? cloneDocument(documentRef.current),
      pointers,
      originPointers: new Map(pointers),
      origin: { x: transform.x, y: transform.y, scale: transform.scale, rotation: transform.rotation },
    };
  };

  const moveGesture = (event: ReactPointerEvent<HTMLElement>) => {
    const gesture = gestureRef.current;
    if (!gesture || !gesture.pointers.has(event.pointerId)) return;
    gesture.pointers.set(event.pointerId, { x: event.clientX, y: event.clientY });
    const currentPoints = [...gesture.pointers.values()];
    const originPoints = [...gesture.originPointers.values()];
    if (gesture.target !== "model") {
      const transform = photoRoomTextTransformFromGesture(
        gesture.origin,
        originPoints,
        currentPoints,
        stageSize,
      );
      liveUpdate((current) => ({
        ...current,
        texts: current.texts.map((text) => text.id === gesture.target ? { ...text, ...transform } : text),
      }));
      return;
    }
    let x = gesture.origin.x;
    let y = gesture.origin.y;
    let scale = gesture.origin.scale;
    let rotation = gesture.origin.rotation;
    if (currentPoints.length === 1 && originPoints.length === 1) {
      x += (currentPoints[0].x - originPoints[0].x) / stageSize.width;
      y += (currentPoints[0].y - originPoints[0].y) / stageSize.height;
    } else if (currentPoints.length >= 2 && originPoints.length >= 2) {
      const currentCenter = { x: (currentPoints[0].x + currentPoints[1].x) / 2, y: (currentPoints[0].y + currentPoints[1].y) / 2 };
      const originCenter = { x: (originPoints[0].x + originPoints[1].x) / 2, y: (originPoints[0].y + originPoints[1].y) / 2 };
      x += (currentCenter.x - originCenter.x) / stageSize.width;
      y += (currentCenter.y - originCenter.y) / stageSize.height;
      const currentDistance = Math.hypot(currentPoints[1].x - currentPoints[0].x, currentPoints[1].y - currentPoints[0].y);
      const originDistance = Math.max(1, Math.hypot(originPoints[1].x - originPoints[0].x, originPoints[1].y - originPoints[0].y));
      scale *= currentDistance / originDistance;
      const currentAngle = Math.atan2(currentPoints[1].y - currentPoints[0].y, currentPoints[1].x - currentPoints[0].x);
      const originAngle = Math.atan2(originPoints[1].y - originPoints[0].y, originPoints[1].x - originPoints[0].x);
      rotation += (currentAngle - originAngle) * 180 / Math.PI;
    }
    liveUpdate((current) => ({ ...current, model: { ...current.model, x, y, scale, rotation } }));
  };

  const endGesture = (event: ReactPointerEvent<HTMLElement>) => {
    const gesture = gestureRef.current;
    if (!gesture) return;
    gesture.pointers.delete(event.pointerId);
    gesture.originPointers.delete(event.pointerId);
    if (gesture.pointers.size) {
      const transform = targetTransform(gesture.target);
      if (transform) {
        gesture.origin = { x: transform.x, y: transform.y, scale: transform.scale, rotation: transform.rotation };
        gesture.originPointers = new Map(gesture.pointers);
      }
      return;
    }
    recordGesture(gesture.before);
    gestureRef.current = undefined;
  };

  const beginTextHandleGesture = (
    event: ReactPointerEvent<HTMLButtonElement>,
    text: PhotoRoomTextLayer,
    mode: TextHandleGestureState["mode"],
  ) => {
    event.preventDefault();
    event.stopPropagation();
    event.currentTarget.setPointerCapture(event.pointerId);
    const stageBounds = stageRef.current?.getBoundingClientRect();
    if (!stageBounds) return;
    setSelectedTextId(text.id);
    textHandleGestureRef.current = {
      mode,
      textId: text.id,
      pointerId: event.pointerId,
      before: cloneDocument(documentRef.current),
      center: {
        x: stageBounds.left + text.x * stageBounds.width,
        y: stageBounds.top + text.y * stageBounds.height,
      },
      originPointer: { x: event.clientX, y: event.clientY },
      originRotation: text.rotation,
      originScale: text.scale,
    };
  };

  const moveTextHandleGesture = (event: ReactPointerEvent<HTMLButtonElement>) => {
    event.stopPropagation();
    const gesture = textHandleGestureRef.current;
    if (!gesture || gesture.pointerId !== event.pointerId) return;
    const currentPointer = { x: event.clientX, y: event.clientY };
    if (gesture.mode === "rotate") {
      const rotation = photoRoomTextRotationFromPointer(
        gesture.originRotation,
        gesture.center,
        gesture.originPointer,
        currentPointer,
        event.shiftKey ? 15 : 0,
      );
      liveUpdate((current) => ({
        ...current,
        texts: current.texts.map((text) => text.id === gesture.textId ? { ...text, rotation } : text),
      }));
      return;
    }
    const scale = photoRoomTextScaleFromPointer(
      gesture.originScale,
      gesture.center,
      gesture.originPointer,
      currentPointer,
    );
    liveUpdate((current) => ({
      ...current,
      texts: current.texts.map((text) => text.id === gesture.textId ? { ...text, scale } : text),
    }));
  };

  const endTextHandleGesture = (event: ReactPointerEvent<HTMLButtonElement>) => {
    event.stopPropagation();
    const gesture = textHandleGestureRef.current;
    if (!gesture || gesture.pointerId !== event.pointerId) return;
    recordGesture(gesture.before);
    textHandleGestureRef.current = undefined;
  };

  const editText = (textId: string) => {
    setSelectedTextId(textId);
    setTool("text");
    requestAnimationFrame(() => {
      textInputRef.current?.focus();
      textInputRef.current?.select();
    });
  };

  const beginRotationGesture = (event: ReactPointerEvent<HTMLElement>, mode: PhotoRoomRotationMode) => {
    event.stopPropagation();
    event.currentTarget.setPointerCapture(event.pointerId);
    setSelectedTextId(undefined);
    const point = { x: event.clientX, y: event.clientY };
    const existing = rotationGestureRef.current?.mode === mode ? rotationGestureRef.current : undefined;
    const pointers = new Map(existing?.pointers ?? []);
    pointers.set(event.pointerId, point);
    rotationGestureRef.current = {
      mode,
      before: existing?.before ?? cloneDocument(documentRef.current),
      pointers,
      originPointers: new Map(pointers),
      origin: {
        yaw: documentRef.current.model.yaw,
        pitch: documentRef.current.model.pitch,
        scale: documentRef.current.model.scale,
      },
    };
  };

  const moveRotationGesture = (event: ReactPointerEvent<HTMLElement>) => {
    const gesture = rotationGestureRef.current;
    if (!gesture || !gesture.pointers.has(event.pointerId)) return;
    gesture.pointers.set(event.pointerId, { x: event.clientX, y: event.clientY });
    const currentPoints = [...gesture.pointers.values()];
    const originPoints = [...gesture.originPointers.values()];
    let delta = { x: 0, y: 0 };
    let scale = gesture.origin.scale;
    if (currentPoints.length === 1 && originPoints.length === 1) {
      delta = { x: currentPoints[0].x - originPoints[0].x, y: currentPoints[0].y - originPoints[0].y };
    } else if (currentPoints.length >= 2 && originPoints.length >= 2) {
      const currentCenter = { x: (currentPoints[0].x + currentPoints[1].x) / 2, y: (currentPoints[0].y + currentPoints[1].y) / 2 };
      const originCenter = { x: (originPoints[0].x + originPoints[1].x) / 2, y: (originPoints[0].y + originPoints[1].y) / 2 };
      delta = { x: currentCenter.x - originCenter.x, y: currentCenter.y - originCenter.y };
      const currentDistance = Math.hypot(currentPoints[1].x - currentPoints[0].x, currentPoints[1].y - currentPoints[0].y);
      const originDistance = Math.max(1, Math.hypot(originPoints[1].x - originPoints[0].x, originPoints[1].y - originPoints[0].y));
      scale *= currentDistance / originDistance;
    }
    const rotation = photoRoomRotationFromDrag(gesture.origin, delta, stageSize, gesture.mode);
    liveUpdate((current) => ({ ...current, model: { ...current.model, ...rotation, scale } }));
  };

  const endRotationGesture = (event: ReactPointerEvent<HTMLElement>) => {
    const gesture = rotationGestureRef.current;
    if (!gesture) return;
    gesture.pointers.delete(event.pointerId);
    gesture.originPointers.delete(event.pointerId);
    if (gesture.pointers.size) {
      gesture.origin = {
        yaw: documentRef.current.model.yaw,
        pitch: documentRef.current.model.pitch,
        scale: documentRef.current.model.scale,
      };
      gesture.originPointers = new Map(gesture.pointers);
      return;
    }
    recordGesture(gesture.before);
    rotationGestureRef.current = undefined;
  };

  const chooseBackgroundPhoto = (file?: File) => {
    if (!file) return;
    if (photoUrlRef.current) URL.revokeObjectURL(photoUrlRef.current);
    const url = URL.createObjectURL(file);
    photoUrlRef.current = url;
    commit((current) => ({ ...current, background: { type: "photo", url, name: file.name } }));
  };

  const addText = () => {
    const layer = createTextLayer(tr("Your text"));
    commit((current) => ({ ...current, texts: [...current.texts, layer] }));
    setSelectedTextId(layer.id);
    setTool("text");
  };

  const updateSelectedText = (changes: Partial<PhotoRoomTextLayer>) => {
    if (!selectedTextId) return;
    commit((current) => ({ ...current, texts: current.texts.map((text) => text.id === selectedTextId ? { ...text, ...changes } : text) }));
  };

  const removeSelectedText = () => {
    if (!selectedTextId) return;
    commit((current) => ({ ...current, texts: current.texts.filter((text) => text.id !== selectedTextId) }));
    setSelectedTextId(undefined);
  };

  const modelCanvas = () => modelRootRef.current?.querySelector("canvas") ?? null;
  const deliverMedia = async (media: PhotoRoomPreparedMedia) => {
    setDelivering(true); setError("");
    try {
      await onDeliver(media);
      setMessage(tr(media.kind === "video" ? "Video sent successfully." : "Photo sent successfully."));
    } catch (reason) {
      if (!(reason instanceof DOMException && reason.name === "AbortError")) setError(tr(reason instanceof Error ? reason.message : String(reason)));
    } finally { setDelivering(false); }
  };
  const preparePhoto = async () => {
    const canvas = exportCanvasRef.current;
    if (!canvas) return;
    const snapshot = cloneDocument(documentRef.current);
    setPreviewing(false);
    setAnimationProgress(0);
    setOrientationOverride(photoRoomModelOrientation(snapshot.model, snapshot.animation, 0));
    setPreparing("photo"); setError(""); setMessage(""); setProgress(0);
    try {
      await nextPaint();
      await drawComposition(canvas, snapshot, modelCanvas());
      const blob = await canvasBlob(canvas, "image/jpeg", 0.94);
      const name = `${safePhotoRoomFilename(title)}-${snapshot.format}.jpg`;
      const media: PhotoRoomPreparedMedia = { kind: "photo", file: new File([blob], name, { type: "image/jpeg" }) };
      setPrepared(media);
      setPreparing(undefined);
      await deliverMedia(media);
    } catch (reason) { setError(tr(reason instanceof Error ? reason.message : String(reason))); }
    finally { setPreparing(undefined); setProgress(0); setOrientationOverride(undefined); }
  };

  const prepareVideo = async () => {
    const canvas = exportCanvasRef.current;
    if (!canvas) return;
    const controller = new AbortController();
    const snapshot = cloneDocument(documentRef.current);
    abortRef.current = controller;
    setPreparing("video"); setPreviewing(false); setAnimationProgress(0); setError(""); setMessage(""); setProgress(0);
    setOrientationOverride(photoRoomModelOrientation(snapshot.model, snapshot.animation, 0));
    try {
      await nextPaint();
      await drawComposition(canvas, snapshot, modelCanvas());
      const posterBlob = await canvasBlob(canvas, "image/jpeg", 0.94);
      const baseName = safePhotoRoomFilename(title);
      const poster = new File([posterBlob], `${baseName}-${snapshot.format}-poster.jpg`, { type: "image/jpeg" });
      const videoBlob = await encodeMp4(
        canvas,
        snapshot,
        async (frame, total) => {
          const frameProgress = frame / total;
          setAnimationProgress(frameProgress);
          setOrientationOverride(photoRoomModelOrientation(snapshot.model, snapshot.animation, frameProgress));
          await nextPaint();
          await drawComposition(canvas, snapshot, modelCanvas());
        },
        setProgress,
        controller.signal,
      );
      setAnimationProgress(0);
      setPrepared({ kind: "video", file: new File([videoBlob], `${baseName}-${snapshot.format}.mp4`, { type: "video/mp4" }), poster });
      setMessage(tr("MP4 ready. Share it now or save its poster to the project."));
    } catch (reason) {
      if (!(reason instanceof DOMException && reason.name === "AbortError")) setError(tr(reason instanceof Error ? reason.message : String(reason)));
    } finally {
      abortRef.current = undefined;
      setPreparing(undefined);
      setProgress(0);
      setAnimationProgress(0);
      setOrientationOverride(undefined);
    }
  };

  const deliver = async () => {
    if (!prepared) return;
    await deliverMedia(prepared);
  };

  const saveProject = async () => {
    if (!prepared || !onSaveToProject) return;
    setSavingProject(true); setError("");
    try {
      await onSaveToProject(prepared.kind === "video" ? prepared.poster! : prepared.file, useAsCover);
      setMessage(tr(prepared.kind === "video" ? "Poster added to the project." : "Photo added to the project."));
    } catch (reason) { setError(tr(reason instanceof Error ? reason.message : String(reason))); }
    finally { setSavingProject(false); }
  };

  const backgroundStyle = useMemo<CSSProperties>(() => {
    if (document.background.type === "solid") return { background: document.background.color };
    if (document.background.type === "gradient") return { background: `linear-gradient(${document.background.angle}deg, ${document.background.from}, ${document.background.to})` };
    return { backgroundImage: `url(${document.background.url})`, backgroundSize: "cover", backgroundPosition: "center" };
  }, [document.background]);

  const modelStyle: CSSProperties = {
    left: `${document.model.x * 100}%`, top: `${document.model.y * 100}%`,
    transform: `translate(-50%, -50%) rotate(${document.model.rotation}deg) scale(${document.model.scale})`,
  };
  const modelOrientation = orientationOverride ?? photoRoomModelOrientation(
    document.model,
    document.animation,
    animationProgress,
  );
  const activeAnimation = ANIMATION_OPTIONS.find((option) => option.type === document.animation.type) ?? ANIMATION_OPTIONS[0];
  const directionLabels = document.animation.type === "flip"
    ? [tr("Forward"), tr("Backward")]
    : document.animation.type === "tumble"
      ? [tr("Forward"), tr("Reverse")]
      : [tr("Clockwise"), tr("Counter-clockwise")];
  const selectedTextRotation = selectedText
    ? ((Math.round(selectedText.rotation) + 180) % 360 + 360) % 360 - 180
    : 0;
  const yawDegrees = Math.round(document.model.yaw * 180 / Math.PI);
  const pitchDegrees = Math.round(document.model.pitch * 180 / Math.PI);
  const setModelView = (yaw: number, pitch = 0) => commit((current) => ({
    ...current,
    model: { ...current.model, yaw, pitch },
  }));
  const resetModel = () => {
    commit((current) => ({
      ...current,
      model: { x: 0.5, y: 0.52, scale: 0.92, rotation: 0, yaw: 0, pitch: 0 },
    }));
    setResetSignal((value) => value + 1);
  };
  const nudgeModelRotation = (event: ReactKeyboardEvent<HTMLElement>, mode: PhotoRoomRotationMode) => {
    const step = event.shiftKey ? 15 : 5;
    if (event.key === "ArrowLeft" || event.key === "ArrowRight") {
      event.preventDefault();
      const direction = event.key === "ArrowLeft" ? -1 : 1;
      commit((current) => ({ ...current, model: { ...current.model, yaw: current.model.yaw + direction * step * Math.PI / 180 } }));
    } else if (mode === "free" && (event.key === "ArrowUp" || event.key === "ArrowDown")) {
      event.preventDefault();
      const direction = event.key === "ArrowUp" ? -1 : 1;
      commit((current) => ({ ...current, model: { ...current.model, pitch: current.model.pitch + direction * step * Math.PI / 180 } }));
    }
  };

  const selectTool = (nextTool: Tool) => {
    setTool(nextTool);
    if (mobile) setPanelExpanded(true);
  };

  const renderPanel = () => {
    if (tool === "format") return <div className="photo-room__format-grid">{PHOTO_ROOM_FORMATS.map((item) => <button key={item.id} className={document.format === item.id ? "is-active" : ""} onClick={() => commit((current) => ({ ...current, format: item.id }))}><span style={{ aspectRatio: `${item.width}/${item.height}` }} /><strong>{tr(item.label)}</strong><small>{tr(item.detail)}</small>{document.format === item.id && <Check />}</button>)}</div>;
    if (tool === "background") return <div className="photo-room__panel-stack"><div className="photo-room__swatches"><button className={`photo-room__photo-swatch ${document.background.type === "photo" ? "is-active" : ""}`} aria-pressed={document.background.type === "photo"} onClick={() => backgroundInputRef.current?.click()}><ImageIcon /><span>{tr("Photo")}</span></button>{SOLID_PRESETS.map((color) => { const active = document.background.type === "solid" && document.background.color === color; return <button key={color} className={active ? "is-active" : ""} aria-pressed={active} aria-label={tr("Use {color} background", { color })} style={{ background: color }} onClick={() => commit((current) => ({ ...current, background: { type: "solid", color } }))} />; })}{BACKGROUND_PRESETS.map((preset) => { const active = document.background.type === "gradient" && document.background.from === preset.from && document.background.to === preset.to && document.background.angle === preset.angle; return <button key={preset.from} className={active ? "is-active" : ""} aria-pressed={active} aria-label={tr("Use gradient background")} style={{ background: `linear-gradient(${preset.angle}deg, ${preset.from}, ${preset.to})` }} onClick={() => commit((current) => ({ ...current, background: { type: "gradient", ...preset } }))} />; })}</div>{document.background.type === "gradient" && <div className="photo-room__field-row"><label><span>{tr("Start")}</span><input type="color" value={document.background.from} onChange={(event) => commit((current) => current.background.type === "gradient" ? { ...current, background: { ...current.background, from: event.target.value } } : current)} /></label><label><span>{tr("End")}</span><input type="color" value={document.background.to} onChange={(event) => commit((current) => current.background.type === "gradient" ? { ...current, background: { ...current.background, to: event.target.value } } : current)} /></label></div>}</div>;
    if (tool === "model") return <div className="photo-room__panel-stack">
      <div className="photo-room__segmented photo-room__model-modes">
        <button className={modelMode === "arrange" ? "is-active" : ""} onClick={() => setModelMode("arrange")}><Move />{tr("Arrange")}</button>
        <button className={modelMode === "turntable" ? "is-active" : ""} onClick={() => setModelMode("turntable")}><RotateCw />{tr("Turntable")}</button>
        <button className={modelMode === "free" ? "is-active" : ""} onClick={() => setModelMode("free")}><Rotate3D />{tr("Free")}</button>
      </div>
      {modelMode !== "arrange" && <>
        <div className="photo-room__view-presets" aria-label={tr("Model view presets")}>
          <button onClick={() => setModelView(0)}>{tr("Start")}</button>
          <button onClick={() => setModelView(-Math.PI / 4)}>45°</button>
          <button onClick={() => setModelView(-Math.PI / 2)}>{tr("Side")}</button>
          <button onClick={() => setModelView(Math.PI)}>{tr("Back")}</button>
          <button onClick={() => setModelView(document.model.yaw, -Math.PI * 7 / 18)}>{tr("Top")}</button>
        </div>
        <label className="photo-room__range"><span>{tr("Turn")} <strong>{yawDegrees}°</strong></span><input aria-label={tr("Model turn angle")} type="range" min="-180" max="180" value={yawDegrees} onPointerDown={beginControlChange} onChange={(event) => liveUpdate((current) => ({ ...current, model: { ...current.model, yaw: Number(event.target.value) * Math.PI / 180 } }))} onPointerUp={endControlChange} onKeyDown={(event) => { if (!event.repeat) beginControlChange(); }} onKeyUp={endControlChange} /></label>
        <label className="photo-room__range"><span>{tr("Tilt")} <strong>{pitchDegrees}°</strong></span><input aria-label={tr("Model tilt angle")} type="range" min="-80" max="80" value={pitchDegrees} onPointerDown={beginControlChange} onChange={(event) => liveUpdate((current) => ({ ...current, model: { ...current.model, pitch: Number(event.target.value) * Math.PI / 180 } }))} onPointerUp={endControlChange} onKeyDown={(event) => { if (!event.repeat) beginControlChange(); }} onKeyUp={endControlChange} /></label>
      </>}
      <label className="photo-room__range"><span>{tr("Model size")} <strong>{Math.round(document.model.scale * 100)}%</strong></span><input aria-label={tr("Model size")} type="range" min="35" max="250" value={Math.round(document.model.scale * 100)} onPointerDown={beginControlChange} onChange={(event) => liveUpdate((current) => ({ ...current, model: { ...current.model, scale: Number(event.target.value) / 100 } }))} onPointerUp={endControlChange} onKeyDown={(event) => { if (!event.repeat) beginControlChange(); }} onKeyUp={endControlChange} /></label>
      <div className="photo-room__button-row"><button onClick={resetModel}>{tr("Reset model")}</button></div>
      <p className="photo-room__help">{modelMode === "arrange"
        ? tr("Drag to move. Pinch to resize and twist.")
        : modelMode === "turntable"
          ? tr("Drag left or right for a controlled turn. Pinch to resize; use Tilt for elevation.")
          : tr("Drag in any direction to turn and tilt. Pinch to resize.")}</p>
    </div>;
    if (tool === "text") return <div className="photo-room__panel-stack">{selectedText ? <>
      <textarea ref={textInputRef} aria-label={tr("Text content")} className="photo-room__text-input" value={selectedText.text} maxLength={240} onFocus={beginControlChange} onBlur={endControlChange} onChange={(event) => liveUpdate((current) => ({ ...current, texts: current.texts.map((text) => text.id === selectedText.id ? { ...text, text: event.target.value } : text) }))} />
      <div className="photo-room__segmented"><button className={selectedText.style === "clean" ? "is-active" : ""} onClick={() => updateSelectedText({ style: "clean" })}>{tr("Clean")}</button><button className={selectedText.style === "glow" ? "is-active" : ""} onClick={() => updateSelectedText({ style: "glow" })}>{tr("Glow")}</button><button className={selectedText.style === "outline" ? "is-active" : ""} onClick={() => updateSelectedText({ style: "outline" })}>{tr("Outline")}</button></div>
      <div className="photo-room__field-row"><label><span>{tr("Font")}</span><select value={selectedText.fontFamily} onChange={(event) => updateSelectedText({ fontFamily: event.target.value as PhotoRoomTextLayer["fontFamily"] })}><option value="sans">{tr("Sans")}</option><option value="serif">{tr("Serif")}</option><option value="mono">{tr("Mono")}</option></select></label><label><span>{tr("Color")}</span><input type="color" value={selectedText.color} onChange={(event) => updateSelectedText({ color: event.target.value })} /></label></div>
      <label className="photo-room__range"><span>{tr("Size")} <strong>{Math.round(selectedText.scale * 100)}%</strong></span><input aria-label={tr("Text size")} type="range" min="35" max="400" value={Math.round(selectedText.scale * 100)} onPointerDown={beginControlChange} onChange={(event) => liveUpdate((current) => ({ ...current, texts: current.texts.map((text) => text.id === selectedText.id ? { ...text, scale: Number(event.target.value) / 100 } : text) }))} onPointerUp={endControlChange} onKeyDown={(event) => { if (!event.repeat) beginControlChange(); }} onKeyUp={endControlChange} /></label>
      <label className="photo-room__range"><span>{tr("Angle")} <strong>{selectedTextRotation}°</strong></span><input aria-label={tr("Text angle")} type="range" min="-180" max="180" value={selectedTextRotation} onPointerDown={beginControlChange} onChange={(event) => liveUpdate((current) => ({ ...current, texts: current.texts.map((text) => text.id === selectedText.id ? { ...text, rotation: Number(event.target.value) } : text) }))} onPointerUp={endControlChange} onKeyDown={(event) => { if (!event.repeat) beginControlChange(); }} onKeyUp={endControlChange} /></label>
      <p className="photo-room__help">{tr("Drag to move. Pinch or use the corner handle to resize. Rotation only changes from the round handle or Angle control.")}</p>
      <button className="photo-room__danger" onClick={removeSelectedText}><Trash2 />{tr("Delete text")}</button>
    </> : <div className="photo-room__empty-tool"><Type /><strong>{tr("Add a message")}</strong><p>{tr("Place a title, caption, or callout over your model.")}</p><button onClick={addText}><Plus />{tr("Add text")}</button></div>}</div>;
    if (tool === "animate") return <div className="photo-room__panel-stack">
      <div className="photo-room__animation-grid" aria-label={tr("Animation axis")}>{ANIMATION_OPTIONS.map((option) => <button key={option.type} className={document.animation.type === option.type ? "is-active" : ""} onClick={() => { setAnimationProgress(0); commit((current) => ({ ...current, animation: { ...current.animation, type: option.type } })); }}><span className={`is-${option.type}`} aria-hidden="true" /><strong>{tr(option.label)}</strong><small>{tr(option.detail)}</small></button>)}</div>
      <div className="photo-room__segmented"><button className={document.animation.duration === 6 ? "is-active" : ""} onClick={() => commit((current) => ({ ...current, animation: { ...current.animation, duration: 6 } }))}>{tr("{count} sec", { count: 6 })}</button><button className={document.animation.duration === 10 ? "is-active" : ""} onClick={() => commit((current) => ({ ...current, animation: { ...current.animation, duration: 10 } }))}>{tr("{count} sec", { count: 10 })}</button><button className={document.animation.duration === 15 ? "is-active" : ""} onClick={() => commit((current) => ({ ...current, animation: { ...current.animation, duration: 15 } }))}>{tr("{count} sec", { count: 15 })}</button></div>
      <div className="photo-room__field-row"><label><span>{tr("Rotations")}</span><select value={document.animation.turns} onChange={(event) => commit((current) => ({ ...current, animation: { ...current.animation, turns: Number(event.target.value) as 1 | 2 } }))}><option value="1">{tr("1 turn")}</option><option value="2">{tr("2 turns")}</option></select></label><label><span>{tr("Direction")}</span><select value={document.animation.direction} onChange={(event) => commit((current) => ({ ...current, animation: { ...current.animation, direction: Number(event.target.value) as 1 | -1 } }))}><option value="1">{directionLabels[0]}</option><option value="-1">{directionLabels[1]}</option></select></label></div>
      <button className="photo-room__preview-button" onClick={() => { setOrientationOverride(undefined); setAnimationProgress(0); setPreviewing((value) => !value); }}>{previewing ? <Square /> : <Play />}{previewing ? tr("Stop preview") : tr("Preview {animation}", { animation: tr(activeAnimation.label).toLocaleLowerCase() })}</button>
      <p className="photo-room__help">{tr("Animation starts from your saved model pose and exports as a silent 1080p H.264 MP4 at 30 fps.")}</p>
    </div>;
    return <div className="photo-room__panel-stack photo-room__export-panel">
      <section className="photo-room__export-choice is-photo">
        <header>
          <span className="photo-room__export-icon"><ImageIcon /></span>
          <div><small>{tr("Instant export")}</small><strong>{tr("Photo")}</strong><p>{tr(format.label)} · {format.width} × {format.height} JPEG</p></div>
        </header>
        <button className="photo-room__export-action" disabled={Boolean(preparing) || delivering} onClick={() => void preparePhoto()}>{preparing === "photo" || delivering ? <LoaderCircle className="spin" /> : mobile ? <Share2 /> : <Download />}{tr(preparing === "photo" ? "Rendering…" : delivering ? "Opening…" : mobile ? "Share photo" : "Save photo")}</button>
      </section>

      <section className={`photo-room__export-choice is-video ${preparing === "video" ? "is-rendering" : ""}`}>
        <header>
          <span className="photo-room__export-icon"><Play /></span>
          <div><small>{tr("Animated export")}</small><strong>{tr("{animation} video", { animation: tr(activeAnimation.label) })}</strong><p>{tr("Silent H.264 MP4")}</p></div>
          <em>{document.animation.duration}s</em>
        </header>
        <div className="photo-room__video-specs" aria-label={tr("Video export details")}><span>{tr(format.label)}</span><span>1080p</span><span>{tr("{count} sec", { count: document.animation.duration })}</span></div>
        {preparing === "video"
          ? <div className="photo-room__video-progress" role="status" aria-live="polite">
            <div><strong>{tr("Rendering video")}</strong><span>{Math.round(progress * 100)}%</span></div>
            <div className="photo-room__progress-track"><span style={{ width: `${progress * 100}%` }} /></div>
            <p>{tr("Keep Photo Room open while each frame is encoded.")}</p>
            <button onClick={() => abortRef.current?.abort()}><X />{tr("Cancel render")}</button>
          </div>
          : <button className="photo-room__export-action" disabled={Boolean(preparing) || delivering} onClick={() => void prepareVideo()}><Sparkles />{tr("Render MP4")}</button>}
      </section>

      {prepared && <section className="photo-room__ready"><header><Check /><div><strong>{prepared.file.name}</strong><small>{(prepared.file.size / 1024 / 1024).toFixed(1)} MB</small></div></header><button className="is-primary" disabled={delivering} onClick={() => void deliver()}>{delivering ? <LoaderCircle className="spin" /> : mobile ? <Share2 /> : <Download />}{tr(mobile ? "Share again" : "Save again")}</button>{canSaveToProject && onSaveToProject && <><label className="photo-room__cover-toggle"><input type="checkbox" checked={useAsCover} onChange={(event) => setUseAsCover(event.target.checked)} /><span>{tr("Use as project cover")}</span></label><button disabled={savingProject} onClick={() => void saveProject()}>{savingProject ? <LoaderCircle className="spin" /> : <Plus />}{tr(prepared.kind === "video" ? "Add poster to project" : "Add photo to project")}</button></>}</section>}
      {message && <p className="photo-room__message is-success"><Check />{message}</p>}
      {error && <p className="photo-room__message is-error">{error}</p>}
    </div>;
  };

  const tools: Array<{ id: Tool; label: string; icon: ReactNode }> = mobile
    ? [
      { id: "format", label: tr("Format"), icon: <Square /> },
      { id: "background", label: tr("Scene"), icon: <ImageIcon /> },
      { id: "model", label: tr("Model"), icon: <Rotate3D /> },
      { id: "text", label: tr("Text"), icon: <Type /> },
      { id: "animate", label: tr("Motion"), icon: <Play /> },
    ]
    : [
      { id: "format", label: tr("Format"), icon: <Square /> },
      { id: "background", label: tr("Background"), icon: <ImageIcon /> },
      { id: "model", label: tr("Model"), icon: <Rotate3D /> },
      { id: "text", label: tr("Text"), icon: <Type /> },
      { id: "animate", label: tr("Animate"), icon: <Play /> },
    ];

  return <div className={`photo-room ${mobile ? "photo-room--mobile" : ""}`}>
    <header className="photo-room__header"><button className="photo-room__back" onClick={onClose} aria-label={tr("Close Photo Room")}><ArrowLeft /></button><div><span>{tr("PHOTO ROOM")}</span><strong>{title}</strong></div><div className="photo-room__history"><button disabled={!past.length} onClick={undo} aria-label={tr("Undo")}><Undo2 /></button><button disabled={!future.length} onClick={redo} aria-label={tr("Redo")}><Redo2 /></button><button className="photo-room__export-shortcut" onClick={() => selectTool("export")}>{mobile ? <span>{tr("Done")}</span> : <><Share2 /><span>{tr("Export")}</span></>}</button></div></header>
    <div className="photo-room__workspace">
      <nav className="photo-room__tools">{tools.map((item) => <button key={item.id} className={tool === item.id ? "is-active" : ""} onClick={() => selectTool(item.id)}>{item.icon}<span>{item.label}</span></button>)}{!mobile && <button className={tool === "export" ? "is-active photo-room__tool-export" : "photo-room__tool-export"} onClick={() => selectTool("export")}><Share2 /><span>{tr("Export")}</span></button>}</nav>
      <main className="photo-room__stage-area" onPointerDown={() => setSelectedTextId(undefined)}>
        <div ref={stageRef} className="photo-room__stage" style={{ aspectRatio: `${format.width}/${format.height}`, ...backgroundStyle }}>
          <div ref={modelRootRef} className={`photo-room__model is-${modelMode}`} style={modelStyle}>{renderModel({ capture: true, orientation: modelOrientation, interactionEnabled: false, resetSignal })}</div>
          {(!mobile || tool === "model") && modelMode === "arrange" && !preparing && !previewing && <div className="photo-room__model-hit" onPointerDown={(event) => beginGesture(event, "model")} onPointerMove={moveGesture} onPointerUp={endGesture} onPointerCancel={endGesture} />}
          {(!mobile || tool === "model") && modelMode !== "arrange" && !preparing && !previewing && <div className={`photo-room__model-rotation-hit is-${modelMode}`} role="slider" aria-label={tr(modelMode === "turntable" ? "Rotate model horizontally" : "Rotate model freely")} aria-valuemin={-180} aria-valuemax={180} aria-valuenow={yawDegrees} tabIndex={0} onKeyDown={(event) => nudgeModelRotation(event, modelMode)} onPointerDown={(event) => beginRotationGesture(event, modelMode)} onPointerMove={moveRotationGesture} onPointerUp={endRotationGesture} onPointerCancel={endRotationGesture} />}
          {document.texts.map((text) => {
            const showChrome = selectedTextId === text.id && !previewing && !preparing;
            return <div key={text.id} className={`photo-room__text ${showChrome ? "is-selected" : ""} ${previewing || preparing ? "is-locked" : ""} is-${text.style}`} style={{ left: `${text.x * 100}%`, top: `${text.y * 100}%`, maxWidth: `${text.maxWidth * 100}%`, color: text.color, fontFamily: fontFamily(text), fontWeight: text.fontWeight, fontSize: `${text.fontSize * Math.min(stageSize.width, stageSize.height)}px`, transform: `translate(-50%, -50%) rotate(${text.rotation}deg) scale(${text.scale})` }} onDoubleClick={() => editText(text.id)} onPointerDown={(event) => beginGesture(event, text.id)} onPointerMove={moveGesture} onPointerUp={endGesture} onPointerCancel={endGesture}>
              <span className="photo-room__text-content">{text.text || tr("Text")}</span>
              {showChrome && <>
                <button className="photo-room__text-handle photo-room__text-rotate" style={{ top: `${-34 / text.scale}px`, transform: `translate(-50%, -50%) scale(${1 / text.scale})` }} aria-label={tr("Rotate text")} onPointerDown={(event) => beginTextHandleGesture(event, text, "rotate")} onPointerMove={moveTextHandleGesture} onPointerUp={endTextHandleGesture} onPointerCancel={endTextHandleGesture}><RotateCw /></button>
                <button className="photo-room__text-handle photo-room__text-resize" style={{ right: `${-12 / text.scale}px`, bottom: `${-12 / text.scale}px`, transform: `translate(50%, 50%) scale(${1 / text.scale})` }} aria-label={tr("Resize text")} onPointerDown={(event) => beginTextHandleGesture(event, text, "resize")} onPointerMove={moveTextHandleGesture} onPointerUp={endTextHandleGesture} onPointerCancel={endTextHandleGesture}><Maximize2 /></button>
              </>}
            </div>;
          })}
          {document.format === "story" && !previewing && !preparing && <div className="photo-room__safe-zone" aria-hidden="true"><span>{tr("Keep important content inside this area")}</span></div>}
          {previewing && <div className="photo-room__animation-badge" aria-live="polite"><span className={`is-${document.animation.type}`} aria-hidden="true" />{tr(activeAnimation.label)}</div>}
        </div>
        <button className="photo-room__stage-caption" onPointerDown={(event) => event.stopPropagation()} onClick={() => selectTool("format")} aria-label={tr("Change format. {format}, {detail}", { format: tr(format.label), detail: tr(format.detail) })}><span>{tr(format.label)}</span><small>{tr(format.detail)}</small></button>
      </main>
      <aside ref={panelRef} className={`photo-room__panel${mobile && !panelExpanded ? " is-collapsed" : ""}${tool === "export" ? " is-export" : ""}`}><header><div><span>{tr("EDIT")}</span><h2>{tr(tool === "format" ? "Choose a format" : tool === "background" ? "Set the scene" : tool === "model" ? "Position the model" : tool === "text" ? "Style text" : tool === "animate" ? "Animate the model" : "Export")}</h2></div>{mobile && <button className="photo-room__panel-toggle" aria-label={tr(panelExpanded ? "Collapse controls" : "Expand controls")} aria-expanded={panelExpanded} onClick={() => setPanelExpanded((expanded) => !expanded)}><ChevronDown /></button>}</header>{renderPanel()}</aside>
    </div>
    <input ref={backgroundInputRef} className="photo-room__hidden-input" type="file" accept="image/jpeg,image/png,image/webp" onChange={(event) => { chooseBackgroundPhoto(event.target.files?.[0]); event.target.value = ""; }} />
    <canvas ref={exportCanvasRef} className="photo-room__export-canvas" aria-hidden="true" />
  </div>;
}
