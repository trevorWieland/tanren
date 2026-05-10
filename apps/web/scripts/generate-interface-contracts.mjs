#!/usr/bin/env node

import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import openapiTS, { astToString } from "openapi-typescript";

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

function isObject(value) {
  return typeof value === "object" && value !== null;
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

async function renderOpenApiTypes(openapi) {
  const ast = await openapiTS(openapi);
  return astToString(ast);
}

function resolveContractSchemaOrder(openapi) {
  const schemas = openapi?.components?.schemas;
  if (!isObject(schemas)) {
    throw new Error("OpenAPI document is missing components.schemas");
  }
  return Object.keys(schemas).sort();
}

function generateFile(openapi, renderedOpenApiTypes, aliasOrder) {
  const schemas = openapi?.components?.schemas;
  if (!isObject(schemas)) {
    throw new Error("OpenAPI document is missing components.schemas");
  }
  const aliases = aliasOrder.map(
    (name) =>
      `export type ${name} = components["schemas"][${JSON.stringify(name)}];`,
  );

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
    "// and openapi-typescript via apps/web/scripts/generate-interface-contracts.mjs",
    "// Do not hand-edit this file.",
    "",
    renderedOpenApiTypes.trimEnd(),
    "",
    "// Re-export component schemas for ergonomic imports in web client code.",
    ...aliases,
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

async function main() {
  const openapi = loadOpenApi();
  const aliasOrder = resolveContractSchemaOrder(openapi);
  const renderedOpenApiTypes = await renderOpenApiTypes(openapi);
  const nextContent = formatTypescript(
    generateFile(openapi, renderedOpenApiTypes, aliasOrder),
  );
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
  await main();
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
}
