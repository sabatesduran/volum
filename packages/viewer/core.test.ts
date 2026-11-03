import { describe, expect, it } from "vitest";
import * as THREE from "three";
import { normalizeObject, viewerObjectBounds, visibleObjectSphere } from "./core";

describe("viewer core", () => {
  it("normalizes geometry around the origin", () => {
    const mesh = new THREE.Mesh(new THREE.BoxGeometry(2, 4, 6));
    mesh.position.set(8, 4, 2);
    const normalized = normalizeObject(mesh);
    const box = new THREE.Box3().setFromObject(normalized);
    expect(box.getCenter(new THREE.Vector3()).length()).toBeLessThan(0.0001);
    expect(Math.max(...box.getSize(new THREE.Vector3()).toArray())).toBeCloseTo(2.4);
  });

  it("fits only visible mesh geometry", () => {
    const group = new THREE.Group();
    group.add(new THREE.Mesh(new THREE.BoxGeometry(2, 2, 2)));
    const hidden = new THREE.Mesh(new THREE.BoxGeometry(100, 100, 100));
    hidden.visible = false;
    group.add(hidden);
    expect(visibleObjectSphere(group)?.radius).toBeCloseTo(Math.sqrt(3));
  });

  it("maps source object bounds through normalization", () => {
    const source = new THREE.Group();
    source.userData.volumObjects = [{ id: 1, triangleCount: 1, bounds: [0, 0, 0, 2, 4, 6], dimensions: [2, 4, 6], center: [1, 2, 3] }];
    source.userData.volumSourceBounds = new THREE.Box3(new THREE.Vector3(0, 0, 0), new THREE.Vector3(2, 4, 6));
    source.add(new THREE.Mesh(new THREE.BoxGeometry(2, 4, 6).translate(1, 2, 3)));
    const normalized = normalizeObject(source);
    expect(viewerObjectBounds(normalized, 1)?.isEmpty()).toBe(false);
  });
});
