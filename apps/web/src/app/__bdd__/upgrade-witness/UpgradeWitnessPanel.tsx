"use client";

import { useState } from "react";
import type { ReactNode } from "react";

const FIXTURE_ACTIONS = [
  "reset",
  "write-file",
  "record-baseline",
  "seed-install",
  "mark-legacy-migration-concern",
  "capture-snapshot",
  "run-upgrade-preview",
  "run-upgrade-apply",
  "assert-no-writes",
  "assert-matches-snapshot",
  "assert-preserves-baseline",
  "assert-replaced-from-baseline",
  "assert-file-missing",
  "last-run",
] as const;

type FixtureAction = (typeof FIXTURE_ACTIONS)[number];

export interface UpgradeWitnessPanelProps {
  readonly runFixtureAction: (
    action: FixtureAction,
    payload: unknown,
  ) => Promise<unknown>;
}

const DEFAULT_FIXTURE_ACTION: FixtureAction = "reset";
const DEFAULT_FIXTURE_PAYLOAD = "{}";

export function UpgradeWitnessPanel({
  runFixtureAction,
}: UpgradeWitnessPanelProps): ReactNode {
  const [fixtureAction, setFixtureAction] = useState<FixtureAction>(
    DEFAULT_FIXTURE_ACTION,
  );
  const [fixturePayload, setFixturePayload] = useState<string>(
    DEFAULT_FIXTURE_PAYLOAD,
  );
  const [fixtureOutput, setFixtureOutput] = useState<string>("");
  const [fixtureBusy, setFixtureBusy] = useState<boolean>(false);

  return (
    <section className="w-full max-w-3xl rounded-md border border-[--color-border] bg-[--color-bg-surface] p-4">
      <h2 className="mb-3 text-lg font-semibold">Upgrade Witness Harness</h2>
      <div className="mb-3 grid gap-3 md:grid-cols-2">
        <label className="flex flex-col gap-1 text-sm">
          <span>Action</span>
          <select
            data-testid="upgrade-witness-action"
            className="rounded border border-[--color-border] bg-[--color-bg] p-2"
            value={fixtureAction}
            onChange={(event) => {
              setFixtureAction(event.target.value as FixtureAction);
            }}
          >
            {FIXTURE_ACTIONS.map((action) => (
              <option key={action} value={action}>
                {action}
              </option>
            ))}
          </select>
        </label>
        <button
          type="button"
          data-testid="upgrade-witness-run"
          className="self-end rounded border border-[--color-border] px-4 py-2 text-sm"
          disabled={fixtureBusy}
          onClick={async () => {
            let payload: unknown;
            try {
              payload = JSON.parse(fixturePayload);
            } catch (reason: unknown) {
              setFixtureOutput(
                `{"ok":false,"error":"invalid-json","detail":${JSON.stringify(
                  reason instanceof Error ? reason.message : String(reason),
                )}}`,
              );
              return;
            }

            setFixtureBusy(true);
            try {
              const result = await runFixtureAction(fixtureAction, payload);
              setFixtureOutput(JSON.stringify(result, null, 2));
            } catch (reason: unknown) {
              setFixtureOutput(
                `{"ok":false,"error":${JSON.stringify(
                  reason instanceof Error ? reason.message : String(reason),
                )}}`,
              );
            } finally {
              setFixtureBusy(false);
            }
          }}
        >
          {fixtureBusy ? "running..." : "run"}
        </button>
      </div>
      <label className="mb-3 flex flex-col gap-1 text-sm">
        <span>Payload JSON</span>
        <textarea
          data-testid="upgrade-witness-payload"
          className="min-h-32 rounded border border-[--color-border] bg-[--color-bg] p-2 font-mono text-xs"
          value={fixturePayload}
          onChange={(event) => {
            setFixturePayload(event.target.value);
          }}
        />
      </label>
      <pre
        data-testid="upgrade-witness-output"
        className="max-h-80 overflow-auto rounded border border-[--color-border] bg-[--color-bg] p-2 text-xs"
      >
        {fixtureOutput}
      </pre>
    </section>
  );
}
