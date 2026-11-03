import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const contract = JSON.parse(readFileSync(new URL("./openapi.json", import.meta.url), "utf8"));

function operations() {
  return Object.entries(contract.paths as Record<string, Record<string, { operationId?: string; responses?: object }>>)
    .flatMap(([path, methods]) => Object.entries(methods)
      .filter(([method]) => method !== "parameters")
      .map(([method, operation]) => ({ path, method, operation })));
}

describe("Companion OpenAPI contract", () => {
  it("has stable unique operation identifiers and documented responses", () => {
    const all = operations();
    const identifiers = all.map(({ operation }) => operation.operationId);

    expect(all.length).toBeGreaterThan(70);
    expect(identifiers.every(Boolean)).toBe(true);
    expect(new Set(identifiers).size).toBe(identifiers.length);
    for (const { path, method, operation } of all) {
      expect(operation.responses, `${method.toUpperCase()} ${path}`).toBeTruthy();
    }
  });

  it("resolves every local schema reference", () => {
    const document = JSON.stringify(contract);
    const references = [...document.matchAll(/"\$ref":"#\/components\/schemas\/([^"]+)"/g)]
      .map((match) => match[1]);
    for (const reference of references) {
      expect(contract.components.schemas[reference], reference).toBeTruthy();
    }
  });

  it("documents independent upload capabilities", () => {
    const capabilities = contract.components.schemas.ServerCapabilities;
    expect(capabilities.required).toContain("organizationWrites");
    expect(capabilities.required).toContain("photoUploads");
    expect(capabilities.required).toContain("fileUploads");
    expect(capabilities.properties.photoUploads.description).toContain("independent");
  });
});
