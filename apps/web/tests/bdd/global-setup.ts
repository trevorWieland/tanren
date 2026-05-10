/* eslint-disable */
// global-setup.ts — boots the Tanren API binary on a free port against
// an ephemeral SQLite database, then exports the URL via
// NEXT_PUBLIC_API_URL so the Next.js dev server (Playwright `webServer`)
// picks it up. Mirrors the `ApiHarness::spawn` shape used by the Rust
// `@api` BDD harness in `crates/tanren-testkit/src/harness/api.rs`.

import { spawn, type ChildProcess } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { setTimeout as delay } from "node:timers/promises";
import { randomBytes } from "node:crypto";

declare global {
  // Stash the spawned process + temp paths on globalThis so
  // global-teardown.ts can clean up.
  // eslint-disable-next-line no-var
  var __tanrenBddState: TanrenBddState | undefined;
}

interface TanrenBddState {
  apiProcess: ChildProcess | null;
  databaseUrl: string;
  databasePath: string;
  tmpRoot: string;
  /**
   * Pre-existing `.env.local` content captured at setup time so
   * `globalTeardown` can restore the developer's file rather than
   * unlinking it. `null` when no `.env.local` existed before our run.
   */
  preExistingEnvLocal: string | null;
}

async function waitForHealth(url: string, timeoutMs: number): Promise<void> {
  const start = Date.now();
  while (Date.now() - start < timeoutMs) {
    try {
      const r = await fetch(url);
      if (r.ok) return;
    } catch {
      // Server not up yet; try again.
    }
    await delay(250);
  }
  throw new Error(
    `Tanren API did not become healthy at ${url} within ${timeoutMs}ms`,
  );
}

async function runCargo(
  cmd: string,
  args: string[],
  cwd: string,
  env: Record<string, string>,
): Promise<void> {
  await new Promise<void>((resolve, reject) => {
    const child = spawn(cmd, args, {
      cwd,
      env: { ...process.env, ...env },
      stdio:
        process.env["TANREN_BDD_API_STDIO"] === "inherit"
          ? "inherit"
          : "ignore",
    });
    child.on("error", reject);
    child.on("exit", (code) => {
      if (code === 0) resolve();
      else reject(new Error(`${cmd} ${args.join(" ")} exited with ${code}`));
    });
  });
}

async function tryPort(preferred: number): Promise<number> {
  const net = await import("node:net");
  return new Promise<number>((resolve) => {
    const srv = net.createServer();
    srv.unref();
    srv.on("error", () => {
      // Port busy — fall back to a kernel-picked free port.
      pickFreePort().then(resolve);
    });
    srv.listen(preferred, "127.0.0.1", () => {
      srv.close(() => resolve(preferred));
    });
  });
}

async function pickFreePort(): Promise<number> {
  const net = await import("node:net");
  return new Promise<number>((resolve, reject) => {
    const srv = net.createServer();
    srv.unref();
    srv.on("error", reject);
    srv.listen(0, "127.0.0.1", () => {
      const addr = srv.address();
      const port = typeof addr === "object" && addr ? addr.port : 0;
      srv.close(() => resolve(port));
    });
  });
}

export default async function globalSetup(): Promise<void> {
  // Capture any pre-existing .env.local so globalTeardown can restore
  // the developer's file.
  const envLocalPath = join(process.cwd(), ".env.local");
  const preExistingEnvLocal = existsSync(envLocalPath)
    ? readFileSync(envLocalPath, "utf-8")
    : null;

  // Generate a per-run secret shared between the API process and the
  // Playwright driver. Never checked into files — lives in process
  // memory and a transient `.env.local` entry for the Next.js dev
  // server (cleaned up by global-teardown).
  const testHookSecret = randomBytes(32).toString("base64url");

  // If the caller already booted an API (e.g. `cargo run -p tanren-api`
  // in another shell), respect that and skip our own spawn.
  if (process.env["TANREN_BDD_EXTERNAL_API"] === "true") {
    if (!process.env["NEXT_PUBLIC_API_URL"]) {
      throw new Error(
        "TANREN_BDD_EXTERNAL_API=true but NEXT_PUBLIC_API_URL is unset",
      );
    }
    process.env["NEXT_PUBLIC_ENABLE_UPGRADE_WITNESS"] = "true";
    process.env["NEXT_PUBLIC_TEST_HOOK_SECRET"] = testHookSecret;
    writeFileSync(
      envLocalPath,
      `NEXT_PUBLIC_API_URL=${process.env["NEXT_PUBLIC_API_URL"]}\nNEXT_PUBLIC_ENABLE_UPGRADE_WITNESS=true\nNEXT_PUBLIC_TEST_HOOK_SECRET=${testHookSecret}\n`,
    );
    globalThis.__tanrenBddState = {
      apiProcess: null,
      databaseUrl: "",
      databasePath: "",
      tmpRoot: "",
      preExistingEnvLocal,
    };
    return;
  }

  const tmpRoot = mkdtempSync(join(tmpdir(), "tanren-bdd-"));
  const databasePath = join(tmpRoot, "bdd.db");
  const databaseUrl = `sqlite://${databasePath}?mode=rwc`;

  const repoRoot =
    process.env["TANREN_REPO_ROOT"] ?? join(process.cwd(), "..", "..");

  await runCargo(
    "cargo",
    ["run", "-q", "-p", "tanren-cli", "--", "migrate", "up"],
    repoRoot,
    { DATABASE_URL: databaseUrl },
  );

  const apiPort = await tryPort(8081);
  const apiUrl = `http://127.0.0.1:${apiPort}`;
  const webPort = process.env["PLAYWRIGHT_WEB_PORT"] ?? "3000";
  const webOrigin = `http://127.0.0.1:${webPort}`;

  const apiProcess = spawn(
    "cargo",
    ["run", "-q", "-p", "tanren-api", "--features", "test-hooks"],
    {
      cwd: repoRoot,
      env: {
        ...process.env,
        DATABASE_URL: databaseUrl,
        TANREN_API_BIND: `127.0.0.1:${apiPort}`,
        TANREN_API_CORS_ORIGINS: webOrigin,
        // Share the per-run secret with the API process so test-hook
        // routes require the matching header on every request.
        TANREN_TEST_HOOK_SECRET: testHookSecret,
        RUST_LOG: process.env["RUST_LOG"] ?? "warn",
      },
      stdio:
        process.env["TANREN_BDD_API_STDIO"] === "inherit"
          ? "inherit"
          : "ignore",
    },
  );
  apiProcess.on("error", (err) => {
    console.error("[playwright-bdd] failed to spawn tanren-api:", err);
  });

  await waitForHealth(`${apiUrl}/health`, 180_000);

  process.env["NEXT_PUBLIC_API_URL"] = apiUrl;
  process.env["NEXT_PUBLIC_ENABLE_UPGRADE_WITNESS"] = "true";
  process.env["NEXT_PUBLIC_TEST_HOOK_SECRET"] = testHookSecret;
  writeFileSync(
    envLocalPath,
    `NEXT_PUBLIC_API_URL=${apiUrl}\nNEXT_PUBLIC_ENABLE_UPGRADE_WITNESS=true\nNEXT_PUBLIC_TEST_HOOK_SECRET=${testHookSecret}\n`,
  );

  globalThis.__tanrenBddState = {
    apiProcess,
    databaseUrl,
    databasePath,
    tmpRoot,
    preExistingEnvLocal,
  };
}
