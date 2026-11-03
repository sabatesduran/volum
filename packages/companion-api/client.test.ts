import { describe, expect, it } from "vitest";
import { CompanionApiError, CompanionClient, type CompanionTransport } from "./client";

describe("CompanionClient", () => {
  it("injects auth and parses JSON", async () => {
    const requests: unknown[] = [];
    const transport: CompanionTransport = {
      request: async (request) => {
        requests.push(request);
        return { status: 200, body: '{"serverId":"server-1"}' };
      },
    };
    await expect(new CompanionClient(transport, "secret").json("/identity")).resolves.toEqual({ serverId: "server-1" });
    expect(requests).toEqual([{ path: "/identity", method: "GET", body: undefined, token: "secret", headers: undefined }]);
  });

  it("normalizes API errors", async () => {
    const client = new CompanionClient({
      request: async () => ({ status: 409, body: '{"error":{"code":"edit_conflict","message":"Changed","retryable":true}}' }),
    });
    await expect(client.json("/api/v1/projects/1")).rejects.toEqual(
      new CompanionApiError("Changed", 409, "edit_conflict", true),
    );
  });
});
