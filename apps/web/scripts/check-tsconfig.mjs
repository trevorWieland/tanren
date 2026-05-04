#!/usr/bin/env node
// check-tsconfig.mjs
//
// Validates that apps/web/tsconfig.json declares the strict-mode flags
// required by the React-TS profile (M3 in the R-0001 remediation stack).
// Wired into `just check-tsconfig`; PR 10 brings the tsconfig itself
// into compliance, after which this script exits 0.
//
// Why a custom Node script rather than `tsc --showConfig`? tsconfig.json
// is JSONC (// comments + trailing commas); a tiny stripper keeps the
// dependency surface at zero — pure node, no devDeps required.

import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";

const __dirname = dirname(fileURLToPath(import.meta.url));
const TSCONFIG_PATH = resolve(__dirname, "..", "tsconfig.json");

// Required compiler options. Each entry is [flag, expectedValue].
const REQUIRED = [
  ["strict", true],
  ["noUncheckedIndexedAccess", true],
  ["noImplicitOverride", true],
  ["exactOptionalPropertyTypes", true],
  ["verbatimModuleSyntax", true],
  ["erasableSyntaxOnly", true],
  ["noPropertyAccessFromIndexSignature", true],
  ["declaration", true],
  ["declarationMap", true],
  ["sourceMap", true],
];

function stripJsonc(src) {
  // Remove // line comments (but preserve those inside strings).
  // Remove /* block comments */.
  // Remove trailing commas before } or ].
  let out = "";
  let i = 0;
  let inString = false;
  let stringChar = "";
  while (i < src.length) {
    const ch = src[i];
    const next = src[i + 1];
    if (inString) {
      out += ch;
      if (ch === "\\") {
        // Escape sequence; copy the next char verbatim.
        out += next ?? "";
        i += 2;
        continue;
      }
      if (ch === stringChar) {
        inString = false;
      }
      i += 1;
      continue;
    }
    if (ch === '"' || ch === "'") {
      inString = true;
      stringChar = ch;
      out += ch;
      i += 1;
      continue;
    }
    if (ch === "/" && next === "/") {
      // Skip to end of line.
      while (i < src.length && src[i] !== "\n") i += 1;
      continue;
    }
    if (ch === "/" && next === "*") {
      i += 2;
      while (i < src.length && !(src[i] === "*" && src[i + 1] === "/")) i += 1;
      i += 2;
      continue;
    }
    out += ch;
    i += 1;
  }
  // Strip trailing commas before } or ].
  out = out.replace(/,(\s*[}\]])/g, "$1");
  return out;
}

async function main() {
  const raw = await readFile(TSCONFIG_PATH, "utf8");
  let parsed;
  try {
    parsed = JSON.parse(stripJsonc(raw));
  } catch (err) {
    console.error(
      `check-tsconfig: failed to parse ${TSCONFIG_PATH}: ${err.message}`,
    );
    process.exit(1);
  }
  const co = parsed.compilerOptions ?? {};
  const missing = [];
  for (const [flag, expected] of REQUIRED) {
    if (co[flag] !== expected) {
      missing.push(
        `  - ${flag}: expected ${JSON.stringify(expected)}, got ${JSON.stringify(co[flag])}`,
      );
    }
  }
  if (missing.length > 0) {
    console.error("check-tsconfig: missing or wrong-valued compilerOptions:");
    for (const line of missing) console.error(line);
    process.exit(1);
  }
  console.log("check-tsconfig: ok");
}

main().catch((err) => {
  console.error(`check-tsconfig: ${err.stack ?? err.message ?? err}`);
  process.exit(1);
});
