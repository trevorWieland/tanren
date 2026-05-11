"use client";

import type { ReactNode } from "react";

import { UpgradeWitnessPanel } from "./UpgradeWitnessPanel";

const API_URL = process.env["NEXT_PUBLIC_API_URL"] ?? "http://localhost:8080";

async function runUpgradeFixtureAction(
  action: string,
  payload: unknown,
): Promise<unknown> {
  let response: Response;
  try {
    const testHookSecret = process.env["NEXT_PUBLIC_TEST_HOOK_SECRET"];
    const headers: Record<string, string> = {
      "content-type": "application/json",
    };
    if (testHookSecret) {
      headers["x-test-hook-secret"] = testHookSecret;
    }
    response = await fetch(`${API_URL}/test-hooks/upgrade-fixture/${action}`, {
      method: "POST",
      headers,
      body: JSON.stringify(payload),
      credentials: "include",
    });
  } catch (cause: unknown) {
    throw new Error(cause instanceof Error ? cause.message : String(cause));
  }

  if (!response.ok) {
    const message = await response.text();
    throw new Error(message === "" ? `HTTP ${response.status}` : message);
  }

  return (await response.json()) as unknown;
}

export default function UpgradeWitnessPage(): ReactNode {
  return (
    <main className="flex min-h-screen flex-col items-center justify-center gap-6 p-8">
      <UpgradeWitnessPanel runFixtureAction={runUpgradeFixtureAction} />
    </main>
  );
}
