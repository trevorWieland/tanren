/* eslint-disable */
// global-setup.ts — boots the Tanren API binary on a free port against
// an ephemeral SQLite database, then exports the URL via VITE_API_URL
// so the Vite dev server (Playwright `webServer`) picks it up. Mirrors
// the `ApiHarness::spawn` shape used by the Rust `@api` BDD harness.

import { spawn, type ChildProcess } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { setTimeout as delay } from "node:timers/promises";

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
  const envLocalPath = join(process.cwd(), ".env.local");
  const preExistingEnvLocal = existsSync(envLocalPath)
    ? readFileSync(envLocalPath, "utf-8")
    : null;

  if (process.env["TANREN_BDD_EXTERNAL_API"] === "true") {
    if (!process.env["VITE_API_URL"]) {
      throw new Error("TANREN_BDD_EXTERNAL_API=true but VITE_API_URL is unset");
    }
    writeFileSync(
      envLocalPath,
      `VITE_API_URL=${process.env["VITE_API_URL"]}\n`,
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

  process.env["VITE_API_URL"] = apiUrl;
  writeFileSync(envLocalPath, `VITE_API_URL=${apiUrl}\n`);

  globalThis.__tanrenBddState = {
    apiProcess,
    databaseUrl,
    databasePath,
    tmpRoot,
    preExistingEnvLocal,
  };
}
