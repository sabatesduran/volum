import { describe, expect, it } from "vitest";
import {
  CompanionError, filenameFromFilePath, isConnectionError, normalizeServerUrl, pairingPayloadWithAddress,
  parseMediaEnvelope, parsePairingPayload, retryCompanionQuery,
} from "./transport";

function mediaEnvelope(status: number, etag: string, contentType: string, body: number[]) {
  const encoder = new TextEncoder();
  const etagBytes = encoder.encode(etag);
  const contentTypeBytes = encoder.encode(contentType);
  const bytes = new Uint8Array(10 + etagBytes.length + contentTypeBytes.length + body.length);
  bytes.set(encoder.encode("VMC1"));
  const view = new DataView(bytes.buffer);
  view.setUint16(4, status, true);
  view.setUint16(6, etagBytes.length, true);
  view.setUint16(8, contentTypeBytes.length, true);
  bytes.set(etagBytes, 10);
  bytes.set(contentTypeBytes, 10 + etagBytes.length);
  bytes.set(body, 10 + etagBytes.length + contentTypeBytes.length);
  return bytes.buffer;
}

describe("normalizeServerUrl", () => {
  it("adds the Companion port to private addresses without an explicit port", () => {
    expect(normalizeServerUrl("100.81.102.15")).toBe("http://100.81.102.15:43821");
    expect(normalizeServerUrl("volum.example.ts.net")).toBe("http://volum.example.ts.net:43821");
  });

  it("normalizes full URLs and preserves explicit ports", () => {
    expect(normalizeServerUrl(" http://100.81.102.15/ ")).toBe("http://100.81.102.15:43821");
    expect(normalizeServerUrl("https://volum.example.ts.net:1234")).toBe("https://volum.example.ts.net:1234");
    expect(normalizeServerUrl("volum.example.com")).toBe("https://volum.example.com");
    expect(normalizeServerUrl("http://192.168.1.20:44000")).toBe("http://192.168.1.20:44000");
  });

  it("accepts raw and bracketed Tailscale IPv6 addresses", () => {
    expect(normalizeServerUrl("fd7a:115c:a1e0::1")).toBe("http://[fd7a:115c:a1e0::1]:43821");
    expect(normalizeServerUrl("[fd7a:115c:a1e0::1]")).toBe("http://[fd7a:115c:a1e0::1]:43821");
  });

  it.each([
    "",
    "http://user:password@100.81.102.15",
    "http://100.81.102.15/projects",
    "http://100.81.102.15?device=other",
    "http://100.81.102.15#fragment"
  ])("rejects non-address input %j", (value) => {
    expect(() => normalizeServerUrl(value)).toThrow();
  });
});

describe("pairing transport", () => {
  const payload = {
    version: 1,
    serverId: "server-1",
    serverName: "Workshop",
    baseUrl: "http://100.81.102.15:43821",
    secret: "one-use-secret",
    expiresAt: "2999-01-01T00:00:00Z",
    transport: "tailscale-http" as const,
  };

  it("accepts legacy and provider-neutral pairing payloads", () => {
    expect(parsePairingPayload(JSON.stringify(payload)).transport).toBe("tailscale-http");
    expect(parsePairingPayload(JSON.stringify({ ...payload, transport: "private-http" })).transport).toBe("private-http");
    expect(parsePairingPayload(JSON.stringify({ ...payload, baseUrl: "https://volum.example.com", transport: "https" })).transport).toBe("https");
  });

  it("lets a scanned payload advertise another route without changing its identity or secret", () => {
    expect(pairingPayloadWithAddress(payload, "https://volum.example.com")).toEqual({
      ...payload,
      baseUrl: "https://volum.example.com",
      transport: "https",
    });
    expect(pairingPayloadWithAddress(payload, "http://192.168.1.20:43821").transport).toBe("private-http");
  });
});

describe("filenameFromFilePath", () => {
  it("extracts desktop and file URL basenames", () => {
    expect(filenameFromFilePath("/Users/me/phone bracket.3mf")).toBe("phone bracket.3mf");
    expect(filenameFromFilePath("file:///private/tmp/phone%20bracket.STL")).toBe("phone bracket.STL");
    expect(filenameFromFilePath("C:\\Models\\part.step")).toBe("part.step");
  });

  it("extracts Android document names from content URIs", () => {
    expect(filenameFromFilePath("content://com.android.providers.downloads.documents/document/primary%3ADownload%2Fgear.stl"))
      .toBe("gear.stl");
  });

  it("ignores URI query strings and fragments", () => {
    expect(filenameFromFilePath("file:///tmp/case.3mf?token=private#preview")).toBe("case.3mf");
  });
});

describe("parseMediaEnvelope", () => {
  it("decodes media metadata without converting the binary body to JSON", () => {
    const response = parseMediaEnvelope(mediaEnvelope(200, "\"revision\"", "image/png", [0, 1, 255]));
    expect(response.status).toBe(200);
    expect(response.etag).toBe("\"revision\"");
    expect(response.contentType).toBe("image/png");
    expect([...new Uint8Array(response.bytes)]).toEqual([0, 1, 255]);
  });

  it("accepts an empty conditional response", () => {
    const response = parseMediaEnvelope(mediaEnvelope(304, "\"revision\"", "image/png", []));
    expect(response.status).toBe(304);
    expect(response.bytes.byteLength).toBe(0);
  });

  it("rejects malformed and body-bearing 304 responses", () => {
    expect(() => parseMediaEnvelope(new Uint8Array([1, 2, 3]).buffer)).toThrow("unreadable media");
    expect(() => parseMediaEnvelope(mediaEnvelope(304, "\"revision\"", "image/png", [1]))).toThrow("unreadable media");
  });
});

describe("connection failures", () => {
  it("recognizes native Tailscale connection errors", () => {
    expect(isConnectionError("Could not reach the Volum server through the selected network.")).toBe(true);
    expect(isConnectionError("Volum Desktop did not respond. Check that it is open and Companion Access is on.")).toBe(true);
    expect(isConnectionError(new CompanionError("Unavailable", 0, "connection_failed"))).toBe(true);
  });

  it("does not delay connection errors with an automatic retry", () => {
    expect(retryCompanionQuery(0, "Could not reach the Volum server through the selected network.")).toBe(false);
    expect(retryCompanionQuery(0, new Error("Temporary server error"))).toBe(true);
    expect(retryCompanionQuery(1, new Error("Temporary server error"))).toBe(false);
  });
});
