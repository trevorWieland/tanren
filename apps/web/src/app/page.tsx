"use client";

import { useEffect, useState } from "react";
import type { ReactNode } from "react";

import * as m from "@/i18n/paraglide/messages";

interface HealthReport {
  status: string;
  version: string;
  contract_version: number;
}

const API_URL = process.env["NEXT_PUBLIC_API_URL"] ?? "";

export default function Home(): ReactNode {
  const [report, setReport] = useState<HealthReport | null>(null);
  const [error, setError] = useState<string | null>(null);

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
    </main>
  );
}
