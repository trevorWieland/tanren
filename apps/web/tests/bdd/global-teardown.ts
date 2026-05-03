/* eslint-disable */
import { rmSync, unlinkSync, writeFileSync } from "node:fs";
import { join } from "node:path";

declare global {
  // eslint-disable-next-line no-var
  var __tanrenBddState:
    | {
        apiProcess: import("node:child_process").ChildProcess | null;
        databaseUrl: string;
        databasePath: string;
        tmpRoot: string;
        /**
         * Pre-existing `.env.local` content snapshotted in globalSetup.
         * Restored on teardown so we don't clobber a developer's local
         * env file. Codex P2 review on PR #133.
         */
        preExistingEnvLocal: string | null;
      }
    | undefined;
}

export default async function globalTeardown(): Promise<void> {
  const state = globalThis.__tanrenBddState;
  const envLocalPath = join(process.cwd(), ".env.local");

  // Restore the pre-existing .env.local if the developer had one;
  // otherwise unlink the file we wrote. Falls back to unconditional
  // unlink if globalSetup didn't run (defensive — should not happen
  // under playwright-bdd's lifecycle).
  if (state?.preExistingEnvLocal !== undefined) {
    if (state.preExistingEnvLocal === null) {
      try {
        unlinkSync(envLocalPath);
      } catch {
        /* file may not exist */
      }
    } else {
      writeFileSync(envLocalPath, state.preExistingEnvLocal);
    }
  } else {
    try {
      unlinkSync(envLocalPath);
    } catch {
      /* file may not exist */
    }
  }

  if (!state) return;
  if (state.apiProcess && state.apiProcess.exitCode === null) {
    // Codex P2 review on PR #133: `child.killed` flips to true the
    // moment a signal is *sent*, not when the child actually exits, so
    // gating SIGKILL on `!killed` after a successful SIGTERM made the
    // SIGKILL path unreachable. A `cargo run -p tanren-api` process
    // that ignores SIGTERM (or hangs in build/teardown) would survive
    // the hook and pollute later runs. Use the actual exit-state probe
    // (`exitCode === null` ⇒ still running) and wait for the exit
    // event (or a deadline) before escalating.
    const proc = state.apiProcess;
    proc.kill("SIGTERM");
    const exitedAfterTerm = await waitForExit(proc, 5_000);
    if (!exitedAfterTerm) {
      proc.kill("SIGKILL");
      await waitForExit(proc, 2_000);
    }
  }
  if (state.tmpRoot) {
    try {
      rmSync(state.tmpRoot, { recursive: true, force: true });
    } catch {
      /* best-effort cleanup */
    }
  }
  globalThis.__tanrenBddState = undefined;
}

/**
 * Resolve to `true` if the child has exited (or is already exited)
 * within `timeoutMs`; `false` if the deadline elapses with the child
 * still running. Avoids racing on `child.killed`, which only reflects
 * signal delivery and not actual process termination.
 */
async function waitForExit(
  child: import("node:child_process").ChildProcess,
  timeoutMs: number,
): Promise<boolean> {
  if (child.exitCode !== null || child.signalCode !== null) return true;
  return new Promise<boolean>((resolve) => {
    const timer = setTimeout(() => resolve(false), timeoutMs);
    child.once("exit", () => {
      clearTimeout(timer);
      resolve(true);
    });
  });
}
