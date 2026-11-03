import { describe, expect, it } from "vitest";
import { parseProjectQrPayload, projectQrPayload } from "./projectQr";

describe("project QR payloads", () => {
  it("round-trips server and project identities without credentials", () => {
    const encoded = projectQrPayload("server-123", "project-456");
    expect(encoded).toBe("volum://project/server-123/project-456");
    expect(parseProjectQrPayload(encoded)).toEqual({ serverId: "server-123", projectId: "project-456" });
    expect(encoded).not.toContain("token");
  });

  it.each(["https://example.com", "volum://other/server/project", "volum://project/only-one", "not a code"])(
    "rejects invalid code %s",
    (value) => expect(() => parseProjectQrPayload(value)).toThrow(),
  );
});
