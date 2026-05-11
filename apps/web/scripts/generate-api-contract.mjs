#!/usr/bin/env node

import { execFileSync } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const webRoot = resolve(scriptDir, "..");
const repoRoot = resolve(webRoot, "..", "..");
const outDir = resolve(webRoot, "src", "app", "lib");
const openapiPath = join(outDir, "api-contract.openapi.gen.json");
const typesPath = join(outDir, "api-contract.gen.ts");
const valibotPath = join(outDir, "api-contract-valibot.gen.ts");
const generatedRelativePaths = [
  "src/app/lib/api-contract.openapi.gen.json",
  "src/app/lib/api-contract.gen.ts",
  "src/app/lib/api-contract-valibot.gen.ts",
];

const HTTP_METHODS = [
  "get",
  "put",
  "post",
  "delete",
  "options",
  "head",
  "patch",
  "trace",
];

function toPascalCase(value) {
  const out = value
    .replace(/[^a-zA-Z0-9]+/g, " ")
    .trim()
    .split(/\s+/)
    .filter(Boolean)
    .map((part) => part[0].toUpperCase() + part.slice(1))
    .join("");
  return out.length > 0 ? out : "Schema";
}

function toCamelCase(value) {
  const pascal = toPascalCase(value);
  return pascal[0].toLowerCase() + pascal.slice(1);
}

function literalValue(value) {
  if (typeof value === "string") {
    return JSON.stringify(value);
  }
  if (
    typeof value === "number" ||
    typeof value === "boolean" ||
    value === null
  ) {
    return String(value);
  }
  return JSON.stringify(value);
}

function schemaToExpr(schema, context, state) {
  if (!schema || typeof schema !== "object") {
    return "v.unknown()";
  }

  if (schema.$ref) {
    const match = /^#\/components\/schemas\/(.+)$/.exec(schema.$ref);
    if (!match) {
      throw new Error(`Unsupported $ref: ${schema.$ref}`);
    }
    const refName = match[1];
    const constName = `${toCamelCase(refName)}ComponentSchema`;
    if (!state.componentQueue.includes(refName)) {
      state.componentQueue.push(refName);
    }
    return constName;
  }

  if (schema.const !== undefined) {
    return `v.literal(${literalValue(schema.const)})`;
  }

  if (Array.isArray(schema.enum) && schema.enum.length > 0) {
    const allStrings = schema.enum.every((value) => typeof value === "string");
    if (allStrings) {
      return `v.picklist(${JSON.stringify(schema.enum)})`;
    }
    return `v.union([${schema.enum
      .map((value) => `v.literal(${literalValue(value)})`)
      .join(", ")}])`;
  }

  if (Array.isArray(schema.type)) {
    const nonNull = schema.type.filter((kind) => kind !== "null");
    if (nonNull.length === 1 && schema.type.includes("null")) {
      const nullable = { ...schema, type: nonNull[0] };
      delete nullable.nullable;
      return `v.nullable(${schemaToExpr(nullable, `${context}Nullable`, state)})`;
    }
    return `v.union([${schema.type
      .map((kind) =>
        schemaToExpr(
          { ...schema, type: kind },
          `${context}${toPascalCase(kind)}`,
          state,
        ),
      )
      .join(", ")}])`;
  }

  if (schema.oneOf) {
    return `v.union([${schema.oneOf
      .map((entry, idx) => schemaToExpr(entry, `${context}OneOf${idx}`, state))
      .join(", ")}])`;
  }

  if (schema.anyOf) {
    return `v.union([${schema.anyOf
      .map((entry, idx) => schemaToExpr(entry, `${context}AnyOf${idx}`, state))
      .join(", ")}])`;
  }

  if (schema.allOf) {
    return `v.intersect([${schema.allOf
      .map((entry, idx) => schemaToExpr(entry, `${context}AllOf${idx}`, state))
      .join(", ")}])`;
  }

  let expr;
  const type = schema.type;

  if (type === "string") {
    const pipeActions = [];
    if (typeof schema.minLength === "number") {
      pipeActions.push(`v.minLength(${schema.minLength})`);
    }
    if (typeof schema.maxLength === "number") {
      pipeActions.push(`v.maxLength(${schema.maxLength})`);
    }
    if (typeof schema.pattern === "string") {
      pipeActions.push(
        `v.regex(new RegExp(${JSON.stringify(schema.pattern)}))`,
      );
    }
    if (schema.format === "email") {
      pipeActions.push("v.email()");
    }
    expr = pipeActions.length
      ? `v.pipe(v.string(), ${pipeActions.join(", ")})`
      : "v.string()";
  } else if (type === "integer") {
    const pipeActions = ["v.integer()"];
    if (typeof schema.minimum === "number") {
      pipeActions.push(`v.minValue(${schema.minimum})`);
    }
    if (typeof schema.maximum === "number") {
      pipeActions.push(`v.maxValue(${schema.maximum})`);
    }
    expr = `v.pipe(v.number(), ${pipeActions.join(", ")})`;
  } else if (type === "number") {
    const pipeActions = [];
    if (typeof schema.minimum === "number") {
      pipeActions.push(`v.minValue(${schema.minimum})`);
    }
    if (typeof schema.maximum === "number") {
      pipeActions.push(`v.maxValue(${schema.maximum})`);
    }
    expr = pipeActions.length
      ? `v.pipe(v.number(), ${pipeActions.join(", ")})`
      : "v.number()";
  } else if (type === "boolean") {
    expr = "v.boolean()";
  } else if (type === "null") {
    expr = "v.null()";
  } else if (type === "array") {
    const itemExpr = schemaToExpr(schema.items ?? {}, `${context}Item`, state);
    expr = `v.array(${itemExpr})`;
  } else if (
    type === "object" ||
    schema.properties ||
    schema.additionalProperties !== undefined
  ) {
    const props = schema.properties ?? {};
    const required = new Set(
      Array.isArray(schema.required) ? schema.required : [],
    );
    const propEntries = Object.entries(props).map(([name, propSchema]) => {
      const propExpr = schemaToExpr(
        propSchema,
        `${context}${toPascalCase(name)}`,
        state,
      );
      return `${JSON.stringify(name)}: ${required.has(name) ? propExpr : `v.exactOptional(${propExpr})`}`;
    });

    if (propEntries.length > 0) {
      const objectFn =
        schema.additionalProperties === false ? "v.strictObject" : "v.object";
      expr = `${objectFn}({\n${propEntries.map((entry) => `  ${entry},`).join("\n")}\n})`;
    } else if (
      schema.additionalProperties &&
      typeof schema.additionalProperties === "object"
    ) {
      const valueExpr = schemaToExpr(
        schema.additionalProperties,
        `${context}AdditionalProperty`,
        state,
      );
      expr = `v.record(v.string(), ${valueExpr})`;
    } else if (schema.additionalProperties === false) {
      expr = "v.strictObject({})";
    } else {
      expr = "v.record(v.string(), v.unknown())";
    }
  } else {
    expr = "v.unknown()";
  }

  if (schema.nullable === true) {
    return `v.nullable(${expr})`;
  }
  return expr;
}

function findJsonSchema(content) {
  if (!content || typeof content !== "object") {
    return null;
  }
  if (content["application/json"]?.schema) {
    return content["application/json"].schema;
  }
  return null;
}

function generateValibot(openapi) {
  const state = {
    componentQueue: [],
    componentLines: [],
    emittedComponents: new Set(),
    operationLines: [],
  };

  const paths = openapi.paths ?? {};
  for (const [path, pathItem] of Object.entries(paths)) {
    for (const method of HTTP_METHODS) {
      const operation = pathItem?.[method];
      if (!operation) {
        continue;
      }
      const opIdRaw = operation.operationId
        ? String(operation.operationId).replace(/_route$/, "")
        : `${method}_${path}`;
      const opId = toCamelCase(opIdRaw);

      const requestSchema = findJsonSchema(operation.requestBody?.content);
      if (requestSchema) {
        const expr = schemaToExpr(requestSchema, `${opId}Request`, state);
        state.operationLines.push(
          `export const ${opId}RequestSchema = ${expr};`,
        );
      }

      const successResponse = ["200", "201", "202", "203", "204"]
        .map((code) => operation.responses?.[code])
        .find(Boolean);
      const responseSchema = findJsonSchema(successResponse?.content);
      if (responseSchema) {
        const expr = schemaToExpr(responseSchema, `${opId}Response`, state);
        state.operationLines.push(
          `export const ${opId}ResponseSchema = ${expr};`,
        );
      }
    }
  }

  const componentSchemas = openapi.components?.schemas ?? {};
  while (state.componentQueue.length > 0) {
    const componentName = state.componentQueue.shift();
    if (!componentName || state.emittedComponents.has(componentName)) {
      continue;
    }
    const componentSchema = componentSchemas[componentName];
    if (!componentSchema) {
      throw new Error(`Missing component schema: ${componentName}`);
    }
    state.emittedComponents.add(componentName);
    const constName = `${toCamelCase(componentName)}ComponentSchema`;
    const expr = schemaToExpr(
      componentSchema,
      `${toCamelCase(componentName)}Component`,
      state,
    );
    state.componentLines.push(`const ${constName} = v.lazy(() => ${expr});`);
  }

  const lines = [
    "/* eslint-disable */",
    "// This file is auto-generated by apps/web/scripts/generate-api-contract.mjs.",
    "// Do not make direct changes to this file.",
    "",
    'import * as v from "valibot";',
    "",
    ...state.componentLines,
    state.componentLines.length > 0 ? "" : "",
    ...state.operationLines,
    "",
  ];
  return `${lines.join("\n")}`;
}

function run() {
  mkdirSync(outDir, { recursive: true });

  const openapiJson = execFileSync(
    "cargo",
    ["run", "-q", "-p", "tanren-api", "--", "--emit-openapi"],
    {
      cwd: repoRoot,
      encoding: "utf8",
    },
  );

  writeFileSync(openapiPath, openapiJson);

  execFileSync(
    "pnpm",
    ["exec", "openapi-typescript", openapiPath, "-o", typesPath],
    {
      cwd: webRoot,
      stdio: "inherit",
    },
  );

  const openapi = JSON.parse(openapiJson);
  const valibotSource = generateValibot(openapi);
  writeFileSync(valibotPath, valibotSource);

  execFileSync(
    "pnpm",
    ["exec", "prettier", "--write", ...generatedRelativePaths],
    {
      cwd: webRoot,
      stdio: "inherit",
    },
  );

  console.log(`generated ${openapiPath}`);
  console.log(`generated ${typesPath}`);
  console.log(`generated ${valibotPath}`);
}

run();
