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
   * Codex P2 review on PR #133.
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
  // Bind to 0 to ask the kernel for a free port, then close. Subject to
  // a race; acceptable here because the surface area is one process.
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
  // the developer's file. Without this, running `pnpm e2e` locally
  // wipes whatever the developer had configured for unrelated
  // local-dev workflows. Codex P2 review on PR #133.
  const envLocalPath = join(process.cwd(), ".env.local");
  const preExistingEnvLocal = existsSync(envLocalPath)
    ? readFileSync(envLocalPath, "utf-8")
    : null;

  // If the caller already booted an API (e.g. `cargo run -p tanren-api`
  // in another shell), respect that and skip our own spawn.
  if (process.env["TANREN_BDD_EXTERNAL_API"] === "true") {
    if (!process.env["NEXT_PUBLIC_API_URL"]) {
      throw new Error(
        "TANREN_BDD_EXTERNAL_API=true but NEXT_PUBLIC_API_URL is unset",
      );
    }
    writeFileSync(
      envLocalPath,
      `NEXT_PUBLIC_API_URL=${process.env["NEXT_PUBLIC_API_URL"]}\nVITE_API_URL=${process.env["NEXT_PUBLIC_API_URL"]}\n`,
    );
    // Stash the pre-existing content for teardown even on the
    // external-API path; teardown reads __tanrenBddState first and
    // falls back to the unconditional behavior if it's missing, so
    // we always set it.
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

  // Apply migrations before the API process starts. The api binary
  // expects an already-migrated DB; without this step, the first
  // sign-up returns 500 ("no such table: accounts").
  await runCargo(
    "cargo",
    ["run", "-q", "-p", "tanren-cli", "--", "migrate", "up"],
    repoRoot,
    { DATABASE_URL: databaseUrl },
  );

  // Use the deterministic 8081 port advertised by the playwright config
  // when it is free; otherwise fall back to a kernel-picked port. The
  // deterministic path means the `playwright.config.ts` `apiUrl` constant
  // (computed at config-load time, before this hook runs) lines up with
  // the URL the API actually listens on.
  const apiPort = await tryPort(8081);
  const apiUrl = `http://127.0.0.1:${apiPort}`;
  const webPort = process.env["PLAYWRIGHT_WEB_PORT"] ?? "3000";
  const webOrigin = `http://127.0.0.1:${webPort}`;

  // Spawn the API with the `test-hooks` feature so the
  // `/test-hooks/*` fixture-seeding routes are available — those are the
  // seam the @web invitation scenarios rely on (Playwright cannot reach
  // `Store::seed_invitation` directly the way the in-process Rust BDD
  // harness can). The feature flag is a passthrough on the binary crate
  // that turns on `tanren-api-app/test-hooks`. Production binaries do
  // not enable this feature and never expose `/test-hooks/*`.
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
        // Quiet the API's tracing output; uncomment for debugging.
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

  // Wait for the API to come up on /health. The first build can take a
  // minute or two; CI bakes the binary into the runner image so the
  // observed latency is just the warm-up.
  await waitForHealth(`${apiUrl}/health`, 180_000);

  process.env["NEXT_PUBLIC_API_URL"] = apiUrl;
  writeFileSync(
    envLocalPath,
    `NEXT_PUBLIC_API_URL=${apiUrl}\nVITE_API_URL=${apiUrl}\n`,
  );

  globalThis.__tanrenBddState = {
    apiProcess,
    databaseUrl,
    databasePath,
    tmpRoot,
    preExistingEnvLocal,
  };
}
