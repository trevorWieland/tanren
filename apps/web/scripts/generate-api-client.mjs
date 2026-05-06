#!/usr/bin/env node

/**
 * Generate TypeScript types from the Tanren API OpenAPI spec.
 *
 * Usage:
 *   node scripts/generate-api-client.mjs [API_URL]
 *
 * Defaults to http://localhost:8080/openapi.json.
 * Requires the API server to be running (e.g. `cargo run -p tanren-api`).
 */

import { execSync } from "node:child_process";
import { mkdirSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const apiUrl = process.argv[2] ?? "http://localhost:8080/openapi.json";
const outputFile = resolve(__dirname, "../src/api/generated/tanren.ts");

mkdirSync(dirname(outputFile), { recursive: true });

const tmpJson = resolve(__dirname, "../tmp-openapi.json");

console.log(`Fetching OpenAPI spec from ${apiUrl}`);
execSync(`curl -sf "${apiUrl}" -o "${tmpJson}"`, { stdio: "inherit" });

console.log("Generating TypeScript types…");
execSync(`npx openapi-typescript "${tmpJson}" -o "${outputFile}"`, {
  cwd: resolve(__dirname, ".."),
  stdio: "inherit",
});

console.log(`Done → ${outputFile}`);
