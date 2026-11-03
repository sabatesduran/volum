import { describe, expect, it } from "vitest";
import {
  clampPhotoRoomDocument,
  createPhotoRoomDocument,
  photoRoomCameraFitDistance,
  photoRoomFormat,
  photoRoomModelOrientation,
  photoRoomRotationFromDrag,
  photoRoomTextRotationFromPointer,
  photoRoomTextScaleFromPointer,
  photoRoomTextTransformFromGesture,
  safePhotoRoomFilename,
  type PhotoRoomAnimationType,
  type PhotoRoomQuaternion,
} from "./core";

function expectEquivalentOrientation(actual: PhotoRoomQuaternion, expected: PhotoRoomQuaternion) {
  const dot = actual.reduce((sum, value, index) => sum + value * expected[index], 0);
  expect(Math.abs(dot)).toBeCloseTo(1, 8);
}

describe("photo room document", () => {
  it("provides social-ready format dimensions", () => {
    expect(photoRoomFormat("square")).toMatchObject({ width: 1080, height: 1080 });
    expect(photoRoomFormat("story")).toMatchObject({ width: 1080, height: 1920 });
  });

  it("clamps transforms into editable limits", () => {
    const document = createPhotoRoomDocument();
    document.model = { x: -2, y: 3, scale: 20, rotation: 0, yaw: Math.PI * 3, pitch: Math.PI };
    expect(clampPhotoRoomDocument(document).model).toEqual({ x: 0, y: 1, scale: 2.5, rotation: 0, yaw: -Math.PI, pitch: Math.PI * 4 / 9 });
  });

  it("keeps turntable drags horizontal and lets free drags tilt", () => {
    const origin = { yaw: 0, pitch: 0.2 };
    expect(photoRoomRotationFromDrag(origin, { x: 150, y: 100 }, { width: 300, height: 400 }, "turntable"))
      .toEqual({ yaw: Math.PI / 2, pitch: 0.2 });
    expect(photoRoomRotationFromDrag(origin, { x: 150, y: 100 }, { width: 300, height: 400 }, "free"))
      .toEqual({ yaw: Math.PI / 2, pitch: 0.2 + Math.PI * 0.1875 });
  });

  it("moves the camera back for narrow reel framing", () => {
    const squareDistance = photoRoomCameraFitDistance(1, 34, 1);
    const reelDistance = photoRoomCameraFitDistance(1, 34, 9 / 16);
    expect(reelDistance).toBeGreaterThan(squareDistance * 1.7);
    expect(photoRoomCameraFitDistance(1, 34, 16 / 9)).toBeCloseTo(squareDistance);
  });

  it.each([
    ["spin", [0, Math.SQRT1_2, 0, Math.SQRT1_2]],
    ["flip", [Math.SQRT1_2, 0, 0, Math.SQRT1_2]],
    ["roll", [0, 0, Math.SQRT1_2, Math.SQRT1_2]],
    ["tumble", [0.5, 0.5, 0, Math.SQRT1_2]],
  ] as Array<[PhotoRoomAnimationType, PhotoRoomQuaternion]>)
  ("animates the model on the %s axis", (type, expected) => {
    expectEquivalentOrientation(
      photoRoomModelOrientation(
        { yaw: 0, pitch: 0 },
        { type, turns: 1, direction: 1 },
        0.25,
      ),
      expected,
    );
  });

  it("starts from the saved pose and closes every animation direction seamlessly", () => {
    const model = { yaw: 0.7, pitch: -0.35 };
    for (const type of ["spin", "flip", "roll", "tumble"] as const) {
      for (const direction of [1, -1] as const) {
        const animation = { type, turns: 1 as const, direction };
        const start = photoRoomModelOrientation(model, animation, 0);
        const end = photoRoomModelOrientation(model, animation, 1);
        expectEquivalentOrientation(end, start);
      }
    }
  });

  it("reverses the selected animation axis", () => {
    const forward = photoRoomModelOrientation(
      { yaw: 0, pitch: 0 },
      { type: "spin", turns: 1, direction: 1 },
      0.25,
    );
    const reverse = photoRoomModelOrientation(
      { yaw: 0, pitch: 0 },
      { type: "spin", turns: 1, direction: -1 },
      0.25,
    );
    expect(forward[1]).toBeCloseTo(-reverse[1]);
    expect(forward[3]).toBeCloseTo(reverse[3]);
  });

  it("moves and pinches text without changing its angle", () => {
    const transform = photoRoomTextTransformFromGesture(
      { x: 0.5, y: 0.4, scale: 1, rotation: 33 },
      [{ x: 0, y: 0 }, { x: 100, y: 0 }],
      [{ x: 0, y: 0 }, { x: 0, y: 200 }],
      { width: 1_000, height: 1_000 },
    );
    expect(transform).toEqual({ x: 0.45, y: 0.5, scale: 2, rotation: 33 });
  });

  it("uses explicit handles for text rotation and resizing", () => {
    const center = { x: 100, y: 100 };
    expect(photoRoomTextRotationFromPointer(0, center, { x: 200, y: 100 }, { x: 100, y: 0 })).toBeCloseTo(-90);
    expect(photoRoomTextRotationFromPointer(4, center, { x: 200, y: 100 }, { x: 198, y: 117 }, 15)).toBe(15);
    expect(photoRoomTextScaleFromPointer(1, center, { x: 150, y: 100 }, { x: 200, y: 100 })).toBe(2);
  });

  it("migrates the old boxed text style to boxless glow", () => {
    const document = createPhotoRoomDocument();
    document.texts = [{
      ...document.texts[0],
      id: "legacy",
      type: "text",
      text: "No box",
      x: 0.5,
      y: 0.5,
      scale: 1,
      rotation: 0,
      fontSize: 0.07,
      fontFamily: "sans",
      fontWeight: 900,
      color: "#ffffff",
      style: "label" as never,
      maxWidth: 0.82,
    }];
    expect(clampPhotoRoomDocument(document).texts[0].style).toBe("glow");
  });

  it("creates safe export names", () => {
    expect(safePhotoRoomFilename('Phone / stand: "final"')).toBe("Phone - stand- -final-");
  });
});
