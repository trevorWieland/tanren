/* eslint-disable */
import { rmSync, unlinkSync } from "node:fs";
import { join } from "node:path";

declare global {
  // eslint-disable-next-line no-var
  var __tanrenBddState:
    | {
        apiProcess: import("node:child_process").ChildProcess | null;
        databaseUrl: string;
        databasePath: string;
        tmpRoot: string;
      }
    | undefined;
}

export default async function globalTeardown(): Promise<void> {
  const state = globalThis.__tanrenBddState;
  try {
    unlinkSync(join(process.cwd(), ".env.test.local"));
  } catch {
    /* file may not exist */
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
  try {
    rmSync(state.tmpRoot, { recursive: true, force: true });
  } catch {
    /* best-effort cleanup */
  }
  globalThis.__tanrenBddState = undefined;
}
