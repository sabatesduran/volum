import { readFile, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const contract = JSON.parse(await readFile(new URL("../openapi.json", import.meta.url), "utf8"));
const schemas = contract.components?.schemas ?? {};
const routeSource = await readFile(new URL("../../../crates/volum-server/src/companion.rs", import.meta.url), "utf8");

function serverRoutes(source) {
  const start = source.indexOf("fn companion_router(");
  const end = source.indexOf("\nasync fn health", start);
  if (start < 0 || end < 0) throw new Error("Could not locate companion_router in volum-server");
  const router = source.slice(start, end);
  const routes = new Map();
  let cursor = 0;
  while ((cursor = router.indexOf(".route(", cursor)) >= 0) {
    const open = cursor + ".route".length;
    const pathMatch = router.slice(open + 1).match(/^\s*"([^"]+)"\s*,/);
    if (!pathMatch) {
      cursor = open + 1;
      continue;
    }
    let index = open;
    let depth = 1;
    let quoted = false;
    let escaped = false;
    while (++index < router.length && depth > 0) {
      const character = router[index];
      if (quoted) {
        if (escaped) escaped = false;
        else if (character === "\\") escaped = true;
        else if (character === '"') quoted = false;
      } else if (character === '"') quoted = true;
      else if (character === "(") depth += 1;
      else if (character === ")") depth -= 1;
    }
    const expression = router.slice(open + 1, index);
    const methods = routes.get(pathMatch[1]) ?? new Set();
    for (const match of expression.matchAll(/(?:axum::routing::)?\b(get|post|put|delete|patch|head)\s*\(/g)) methods.add(match[1]);
    for (const match of expression.matchAll(/\.(get|post|put|delete|patch|head)\s*\(/g)) methods.add(match[1]);
    routes.set(pathMatch[1], methods);
    cursor = index;
  }
  return routes;
}

function checkRouteDrift() {
  const actual = serverRoutes(routeSource);
  const documented = new Map(Object.entries(contract.paths ?? {}).map(([path, operations]) => [
    path,
    new Set(Object.keys(operations).filter((method) => method !== "parameters")),
  ]));
  const failures = [];
  for (const [path, methods] of actual) {
    if (!documented.has(path)) failures.push(`OpenAPI is missing server route ${path}`);
    for (const method of methods) {
      if (!documented.get(path)?.has(method)) failures.push(`OpenAPI is missing ${method.toUpperCase()} ${path}`);
    }
  }
  for (const [path, methods] of documented) {
    if (!actual.has(path)) failures.push(`OpenAPI documents removed server route ${path}`);
    for (const method of methods) {
      if (!actual.get(path)?.has(method)) failures.push(`OpenAPI documents removed operation ${method.toUpperCase()} ${path}`);
    }
  }
  if (failures.length) throw new Error(failures.join("\n"));
}

function referenceName(reference) {
  return reference.split("/").at(-1);
}

function typeFor(schema, indent = "") {
  if (schema.$ref) return referenceName(schema.$ref);
  if (schema.enum) return schema.enum.map((value) => JSON.stringify(value)).join(" | ");
  if (schema.type === "array") return `Array<${typeFor(schema.items ?? {})}>`;
  if (schema.type === "object" || schema.properties) {
    const required = new Set(schema.required ?? []);
    const rows = Object.entries(schema.properties ?? {}).map(([name, value]) =>
      `${indent}  ${JSON.stringify(name)}${required.has(name) ? "" : "?"}: ${typeFor(value, `${indent}  `)};`);
    return rows.length ? `{\n${rows.join("\n")}\n${indent}}` : "Record<string, unknown>";
  }
  if (schema.type === "integer" || schema.type === "number") return "number";
  if (schema.type === "boolean") return "boolean";
  if (schema.type === "null") return "null";
  return "string";
}

const lines = [
  "// Generated from openapi.json by scripts/generate.mjs. Do not edit by hand.",
  "",
  ...Object.entries(schemas).flatMap(([name, schema]) => [
    `export type ${name} = ${typeFor(schema)};`,
    "",
  ]),
];
const output = `${lines.join("\n").trim()}\n`;
const target = new URL("../generated.ts", import.meta.url);

if (process.argv.includes("--check")) {
  checkRouteDrift();
  const current = await readFile(target, "utf8").catch(() => "");
  if (current !== output) {
    console.error("packages/companion-api/generated.ts is out of date; run npm run generate.");
    process.exitCode = 1;
  }
} else {
  await writeFile(target, output);
  console.log(`Generated ${Object.keys(schemas).length} schemas in ${root}/generated.ts`);
}
