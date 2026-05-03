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
  if (state.apiProcess && !state.apiProcess.killed) {
    state.apiProcess.kill("SIGTERM");
    // Give it a moment to flush; Playwright already runs this after all
    // tests complete so a tight loop is fine.
    await new Promise<void>((resolve) => setTimeout(resolve, 250));
    if (!state.apiProcess.killed) {
      state.apiProcess.kill("SIGKILL");
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
