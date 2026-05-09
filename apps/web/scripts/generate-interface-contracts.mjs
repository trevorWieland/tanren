#!/usr/bin/env node

import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const WEB_ROOT = resolve(__dirname, "..");
const REPO_ROOT = resolve(WEB_ROOT, "..", "..");
const OUTPUT_PATH = resolve(
  WEB_ROOT,
  "src",
  "app",
  "lib",
  "generated-interface-contracts.ts",
);
const CHECK_MODE = process.argv.includes("--check");

const ROOT_SCHEMAS = [
  "MyAccountCapabilitiesResponse",
  "PermissionGrantSource",
  "PermissionConstraintView",
  "MyPermissionEntry",
  "MyOrganizationPermissions",
  "MyProjectPermissions",
  "MyPermissionsPageMeta",
  "MyPermissionsResponse",
  "InterfaceErrorCode",
  "InterfaceError",
];

function refName(ref) {
  return ref.replace("#/components/schemas/", "");
}

function isObject(value) {
  return typeof value === "object" && value !== null;
}

function renderTypeExpr(schema) {
  if (!isObject(schema)) {
    return "unknown";
  }
  if (schema.$ref) {
    return refName(schema.$ref);
  }
  if (Array.isArray(schema.oneOf)) {
    return schema.oneOf.map((entry) => renderTypeExpr(entry)).join(" | ");
  }
  if (Array.isArray(schema.anyOf)) {
    return schema.anyOf.map((entry) => renderTypeExpr(entry)).join(" | ");
  }
  if (Array.isArray(schema.allOf)) {
    return schema.allOf.map((entry) => renderTypeExpr(entry)).join(" & ");
  }
  if (Array.isArray(schema.enum)) {
    return schema.enum.map((entry) => JSON.stringify(entry)).join(" | ");
  }
  if (schema.type === "null") {
    return "null";
  }
  if (schema.type === "string") {
    return "string";
  }
  if (schema.type === "integer" || schema.type === "number") {
    return "number";
  }
  if (schema.type === "boolean") {
    return "boolean";
  }
  if (schema.type === "array") {
    return `${renderTypeExpr(schema.items)}[]`;
  }
  if (schema.type === "object") {
    const properties = isObject(schema.properties) ? schema.properties : {};
    const required = new Set(
      Array.isArray(schema.required) ? schema.required : [],
    );
    const lines = Object.entries(properties).map(([name, propertySchema]) => {
      const optional = required.has(name) ? "" : "?";
      return `  ${name}${optional}: ${renderTypeExpr(propertySchema)};`;
    });
    if (lines.length === 0) {
      return "Record<string, never>";
    }
    return `{\n${lines.join("\n")}\n}`;
  }
  return "unknown";
}

function collectSchemaReferences(schema, refs) {
  if (!isObject(schema)) {
    return;
  }
  if (schema.$ref) {
    refs.add(refName(schema.$ref));
    return;
  }
  if (Array.isArray(schema.oneOf)) {
    for (const entry of schema.oneOf) {
      collectSchemaReferences(entry, refs);
    }
  }
  if (Array.isArray(schema.anyOf)) {
    for (const entry of schema.anyOf) {
      collectSchemaReferences(entry, refs);
    }
  }
  if (Array.isArray(schema.allOf)) {
    for (const entry of schema.allOf) {
      collectSchemaReferences(entry, refs);
    }
  }
  if (isObject(schema.properties)) {
    for (const propertySchema of Object.values(schema.properties)) {
      collectSchemaReferences(propertySchema, refs);
    }
  }
  if (isObject(schema.items)) {
    collectSchemaReferences(schema.items, refs);
  }
  if (isObject(schema.additionalProperties)) {
    collectSchemaReferences(schema.additionalProperties, refs);
  }
}

function resolveRenderOrder(schemas) {
  const seen = new Set();
  const queue = [...ROOT_SCHEMAS];
  while (queue.length > 0) {
    const name = queue.shift();
    if (seen.has(name)) {
      continue;
    }
    seen.add(name);
    const schema = schemas[name];
    if (!isObject(schema)) {
      throw new Error(`OpenAPI document is missing schema: ${name}`);
    }
    const refs = new Set();
    collectSchemaReferences(schema, refs);
    for (const ref of refs) {
      if (!seen.has(ref)) {
        queue.push(ref);
      }
    }
  }
  const dependencies = [...seen]
    .filter((name) => !ROOT_SCHEMAS.includes(name))
    .sort();
  return [...ROOT_SCHEMAS, ...dependencies];
}

function renderNamedSchema(name, schema) {
  if (!isObject(schema)) {
    return `export type ${name} = unknown;`;
  }
  if (schema.type === "object" && !Array.isArray(schema.oneOf)) {
    const properties = isObject(schema.properties) ? schema.properties : {};
    const required = new Set(
      Array.isArray(schema.required) ? schema.required : [],
    );
    const lines = Object.entries(properties).map(
      ([propertyName, propertySchema]) => {
        const optional = required.has(propertyName) ? "" : "?";
        const typeExpr = renderTypeExpr(propertySchema);
        return `  ${propertyName}${optional}: ${typeExpr};`;
      },
    );
    const body = lines.length > 0 ? lines.join("\n") : "  // Empty schema.";
    return `export interface ${name} {\n${body}\n}`;
  }
  return `export type ${name} = ${renderTypeExpr(schema)};`;
}

function loadOpenApi() {
  const tempDir = mkdtempSync(join(tmpdir(), "tanren-openapi-"));
  const tempOpenApiPath = resolve(tempDir, "openapi.json");
  execFileSync(
    "cargo",
    [
      "run",
      "-q",
      "-p",
      "tanren-xtask",
      "--",
      "export-openapi",
      "--out",
      tempOpenApiPath,
    ],
    { cwd: REPO_ROOT, stdio: "inherit" },
  );
  const raw = readFileSync(tempOpenApiPath, "utf8");
  return JSON.parse(raw);
}

function formatTypescript(source) {
  return execFileSync("pnpm", ["exec", "prettier", "--parser", "typescript"], {
    cwd: WEB_ROOT,
    encoding: "utf8",
    input: source,
  });
}

function generateFile(openapi) {
  const schemas = openapi?.components?.schemas;
  if (!isObject(schemas)) {
    throw new Error("OpenAPI document is missing components.schemas");
  }

  const renderOrder = resolveRenderOrder(schemas);
  const rendered = renderOrder.map((name) => {
    const schema = schemas[name];
    if (!isObject(schema)) {
      throw new Error(`OpenAPI document is missing schema: ${name}`);
    }
    return renderNamedSchema(name, schema);
  });

  const interfaceErrorCodeSchema = schemas.InterfaceErrorCode;
  if (
    !isObject(interfaceErrorCodeSchema) ||
    !Array.isArray(interfaceErrorCodeSchema.enum)
  ) {
    throw new Error("InterfaceErrorCode schema is missing enum metadata");
  }
  const codes = interfaceErrorCodeSchema.enum.map((entry) =>
    JSON.stringify(entry),
  );

  return [
    "// Generated from Tanren's utoipa OpenAPI contract via:",
    "//   cargo run -q -p tanren-xtask -- export-openapi --out <path>",
    "// and apps/web/scripts/generate-interface-contracts.mjs",
    "// Do not hand-edit this file.",
    "",
    ...rendered,
    "",
    `export const INTERFACE_ERROR_CODES = [${codes.join(", ")}] as const;`,
    "",
    "const INTERFACE_ERROR_CODE_SET = new Set<string>(INTERFACE_ERROR_CODES);",
    "",
    "export function isInterfaceErrorCode(value: string): value is InterfaceErrorCode {",
    "  return INTERFACE_ERROR_CODE_SET.has(value);",
    "}",
    "",
  ].join("\n");
}

function main() {
  const openapi = loadOpenApi();
  const nextContent = formatTypescript(generateFile(openapi));
  const currentContent = readFileSync(OUTPUT_PATH, "utf8");
  if (CHECK_MODE) {
    if (currentContent !== nextContent) {
      throw new Error(
        "generated-interface-contracts.ts is out of date. Run: pnpm --filter @tanren/web run contracts:generate",
      );
    }
    return;
  }
  if (currentContent !== nextContent) {
    writeFileSync(OUTPUT_PATH, nextContent, "utf8");
  }
}

try {
  main();
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
}
