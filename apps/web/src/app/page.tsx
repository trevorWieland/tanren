"use client";

import { useEffect, useState } from "react";
import type { ReactNode } from "react";

import {
  type UpgradeFixtureAction,
  runUpgradeFixtureAction,
} from "@/app/lib/account-client";
import * as m from "@/i18n/paraglide/messages";

interface HealthReport {
  status: string;
  version: string;
  contract_version: number;
}

const API_URL = process.env["NEXT_PUBLIC_API_URL"] ?? "http://localhost:8080";
const ENABLE_UPGRADE_WITNESS =
  process.env["NEXT_PUBLIC_ENABLE_UPGRADE_WITNESS"] === "true";
const DEFAULT_FIXTURE_ACTION: UpgradeFixtureAction = "reset";
const DEFAULT_FIXTURE_PAYLOAD = "{}";

export default function Home(): ReactNode {
  const [report, setReport] = useState<HealthReport | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [fixtureAction, setFixtureAction] = useState<UpgradeFixtureAction>(
    DEFAULT_FIXTURE_ACTION,
  );
  const [fixturePayload, setFixturePayload] = useState<string>(
    DEFAULT_FIXTURE_PAYLOAD,
  );
  const [fixtureOutput, setFixtureOutput] = useState<string>("");
  const [fixtureBusy, setFixtureBusy] = useState<boolean>(false);

  useEffect(() => {
    let cancelled = false;
    fetch(`${API_URL}/health`, { credentials: "include" })
      .then(async (response) => {
        if (!response.ok) {
          throw new Error(`HTTP ${response.status}`);
        }
        return (await response.json()) as HealthReport;
      })
      .then((data) => {
        if (!cancelled) {
          setReport(data);
        }
      })
      .catch((reason: unknown) => {
        if (!cancelled) {
          setError(reason instanceof Error ? reason.message : String(reason));
        }
      });
    return () => {
      cancelled = true;
    };
  }, []);

  return (
    <main className="flex min-h-screen flex-col items-center justify-center gap-6 p-8">
      <h1 className="text-3xl font-semibold">{m.app_title()}</h1>
      <p className="text-[--color-fg-muted]">{m.app_placeholder()}</p>
      <section className="min-w-[20rem] rounded-md border border-[--color-border] bg-[--color-bg-surface] px-6 py-4 font-mono">
        {report !== null ? (
          <pre className="m-0">{JSON.stringify(report, null, 2)}</pre>
        ) : error !== null ? (
          <span className="text-[--color-error]">
            {m.app_health_unreachable()}: {error}
          </span>
        ) : (
          <span className="text-[--color-fg-muted]">
            {m.app_health_loading()}
          </span>
        )}
      </section>
      {ENABLE_UPGRADE_WITNESS ? (
        <section className="w-full max-w-3xl rounded-md border border-[--color-border] bg-[--color-bg-surface] p-4">
          <h2 className="mb-3 text-lg font-semibold">
            Upgrade Witness Harness
          </h2>
          <div className="mb-3 grid gap-3 md:grid-cols-2">
            <label className="flex flex-col gap-1 text-sm">
              <span>Action</span>
              <select
                data-testid="upgrade-witness-action"
                className="rounded border border-[--color-border] bg-[--color-bg] p-2"
                value={fixtureAction}
                onChange={(event) => {
                  setFixtureAction(event.target.value as UpgradeFixtureAction);
                }}
              >
                <option value="reset">reset</option>
                <option value="write-file">write-file</option>
                <option value="record-baseline">record-baseline</option>
                <option value="seed-install">seed-install</option>
                <option value="mark-legacy-migration-concern">
                  mark-legacy-migration-concern
                </option>
                <option value="capture-snapshot">capture-snapshot</option>
                <option value="run-upgrade-preview">run-upgrade-preview</option>
                <option value="run-upgrade-apply">run-upgrade-apply</option>
                <option value="assert-no-writes">assert-no-writes</option>
                <option value="assert-matches-snapshot">
                  assert-matches-snapshot
                </option>
                <option value="assert-preserves-baseline">
                  assert-preserves-baseline
                </option>
                <option value="assert-replaced-from-baseline">
                  assert-replaced-from-baseline
                </option>
                <option value="assert-file-missing">assert-file-missing</option>
                <option value="last-run">last-run</option>
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
                  const result = await runUpgradeFixtureAction(
                    fixtureAction,
                    payload,
                  );
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
      ) : null}
    </main>
  );
}
