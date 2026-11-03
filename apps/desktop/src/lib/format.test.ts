import { describe, expect, it } from "vitest";
import { formatBytes, formatDimensions } from "./format";

describe("format helpers", () => {
  it("formats storage without losing the unit", () => {
    expect(formatBytes(1_572_864)).toBe("1.5 MB");
  });

  it("formats model dimensions in base units", () => {
    expect(formatDimensions([80.2, 42.4, 64.8])).toBe("80 × 42 × 65 mm");
  });
});
