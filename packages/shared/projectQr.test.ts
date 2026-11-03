import { describe, expect, it } from "vitest";
import { parseProjectQrPayload, projectQrPayload } from "./projectQr";

describe("project QR payload", () => {
  it("round trips opaque identifiers", () => {
    expect(parseProjectQrPayload(projectQrPayload("server 1", "project/2"))).toEqual({
      serverId: "server 1",
      projectId: "project/2",
    });
  });

  it("rejects credentials and unrelated URLs", () => {
    expect(() => parseProjectQrPayload("https://example.com/project/1")).toThrow();
    expect(() => projectQrPayload("server\nsecret", "project")).toThrow();
  });
});
