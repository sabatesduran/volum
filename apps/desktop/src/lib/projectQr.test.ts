import { describe, expect, it } from "vitest";
import { projectQrPayload } from "./projectQr";

describe("desktop project QR payloads", () => {
  it("matches the mobile project handoff contract without credentials", () => {
    const encoded = projectQrPayload("server-123", "project-456");
    expect(encoded).toBe("volum://project/server-123/project-456");
    expect(encoded).not.toContain("token");
  });

  it.each([["", "project"], ["server", ""], ["server\n", "project"]])(
    "rejects invalid identities",
    (serverId, projectId) => expect(() => projectQrPayload(serverId, projectId)).toThrow(),
  );
});
