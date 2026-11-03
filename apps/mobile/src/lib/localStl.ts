import { invoke } from "@tauri-apps/api/core";

export interface LocalStlFile {
  cachePath: string;
  filename: string;
  byteSize: number;
}

export interface LocalStlPreview {
  payload: ArrayBuffer;
  triangleCount: number;
  dimensions: [number, number, number];
  objects: LocalStlObject[];
}

export interface LocalStlObject {
  id: number;
  name?: string;
  triangleCount: number;
  bounds: [number, number, number, number, number, number];
  dimensions: [number, number, number];
  center: [number, number, number];
}

export interface OpenedStl {
  id: string;
}

export function takeOpenedStls(): Promise<OpenedStl[]> {
  return invoke<OpenedStl[]>("companion_take_opened_stls");
}

export function stageSelectedStl(filePath: string, suggestedFilename: string): Promise<LocalStlFile> {
  return invoke<LocalStlFile>("companion_stage_local_stl", {
    request: { filePath, suggestedFilename },
  });
}

export function stageOpenedStl(openedId: string): Promise<LocalStlFile> {
  return invoke<LocalStlFile>("companion_stage_local_stl", {
    request: { openedId },
  });
}

export async function previewLocalStl(cachePath: string): Promise<LocalStlPreview> {
  const response = await invoke<ArrayBuffer | Uint8Array>("companion_preview_local_stl", { cachePath });
  return parseLocalPreviewEnvelope(response);
}

export function removeLocalStl(cachePath: string): Promise<boolean> {
  return invoke<boolean>("companion_remove_local_stl", { cachePath });
}

export function parseLocalPreviewEnvelope(response: ArrayBuffer | Uint8Array): LocalStlPreview {
  const envelope = response instanceof ArrayBuffer
    ? response
    : response.buffer.slice(response.byteOffset, response.byteOffset + response.byteLength) as ArrayBuffer;
  const bytes = new Uint8Array(envelope);
  if (bytes.byteLength < 44 || String.fromCharCode(...bytes.subarray(0, 4)) !== "VLP1") {
    throw new Error("The local STL preview is invalid.");
  }
  const view = new DataView(envelope);
  const triangleCount = view.getUint32(4, true);
  const payload = envelope.slice(8);
  const payloadBytes = new Uint8Array(payload);
  const version = String.fromCharCode(...payloadBytes.subarray(0, 4));
  if (version !== "VLM4" && version !== "VLM5") {
    throw new Error("The local STL preview is invalid.");
  }
  const payloadView = new DataView(payload);
  const bounds = Array.from({ length: 6 }, (_, index) => payloadView.getFloat32(12 + index * 4, true));
  if (!triangleCount || !bounds.every(Number.isFinite)) {
    throw new Error("The local STL preview is invalid.");
  }
  const dimensions: [number, number, number] = [
    bounds[3] - bounds[0],
    bounds[4] - bounds[1],
    bounds[5] - bounds[2],
  ];
  if (dimensions.some((value) => value < 0 || !Number.isFinite(value))) {
    throw new Error("The local STL preview is invalid.");
  }
  const objects: LocalStlObject[] = [];
  if (version === "VLM5") {
    if (payload.byteLength < 40) throw new Error("The local STL preview is invalid.");
    const objectCount = payloadView.getUint32(36, true);
    const headerLength = 40 + objectCount * 32;
    if (objectCount > 4_096 || headerLength > payload.byteLength) {
      throw new Error("The local STL preview is invalid.");
    }
    let objectTriangles = 0;
    for (let index = 0; index < objectCount; index += 1) {
      const offset = 40 + index * 32;
      const id = payloadView.getUint32(offset, true);
      const objectTriangleCount = payloadView.getUint32(offset + 4, true);
      const objectBounds = Array.from({ length: 6 }, (_, bound) => payloadView.getFloat32(offset + 8 + bound * 4, true)) as LocalStlObject["bounds"];
      if (id !== index + 1 || !objectTriangleCount || !objectBounds.every(Number.isFinite)
        || objectBounds[3] < objectBounds[0] || objectBounds[4] < objectBounds[1] || objectBounds[5] < objectBounds[2]) {
        throw new Error("The local STL preview is invalid.");
      }
      const objectDimensions: LocalStlObject["dimensions"] = [
        objectBounds[3] - objectBounds[0],
        objectBounds[4] - objectBounds[1],
        objectBounds[5] - objectBounds[2],
      ];
      objects.push({
        id,
        triangleCount: objectTriangleCount,
        bounds: objectBounds,
        dimensions: objectDimensions,
        center: [
          (objectBounds[0] + objectBounds[3]) * 0.5,
          (objectBounds[1] + objectBounds[4]) * 0.5,
          (objectBounds[2] + objectBounds[5]) * 0.5,
        ],
      });
      objectTriangles += objectTriangleCount;
    }
    if (objects.length && objectTriangles !== triangleCount) {
      throw new Error("The local STL preview is invalid.");
    }
  }
  return { payload, triangleCount, dimensions, objects };
}
