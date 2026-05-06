"use client";

import { useEffect, useState } from "react";
import type { ReactNode } from "react";
import Link from "next/link";

import * as m from "@/i18n/paraglide/messages";
import { useOrganizationList } from "@/lib/use-organization-queries";

interface HealthReport {
  status: string;
  version: string;
  contract_version: number;
}

const API_URL = process.env["NEXT_PUBLIC_API_URL"] ?? "http://localhost:8080";

export default function Home(): ReactNode {
  const [report, setReport] = useState<HealthReport | null>(null);
  const [error, setError] = useState<string | null>(null);

  const orgQuery = useOrganizationList();
  const organizations = orgQuery.data?.organizations ?? null;

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
      {organizations !== null ? (
        <section className="w-full max-w-lg">
          <div className="mb-4 flex items-center justify-between">
            <h2 className="text-xl font-semibold">{m.orgDashboard_title()}</h2>
            <Link
              href="/organizations/new"
              className="rounded-md border border-[--color-border] bg-[--color-accent] px-3 py-1 text-sm font-medium text-[--color-accent-fg] transition-colors hover:bg-[--color-accent-hover]"
            >
              {m.orgDashboard_createLink()}
            </Link>
          </div>
          {organizations.length > 0 ? (
            <ul className="flex flex-col gap-2">
              {organizations.map((org) => (
                <li
                  key={org.id}
                  data-testid={`org-${org.id}`}
                  className="rounded-md border border-[--color-border] bg-[--color-bg-surface] px-4 py-3"
                >
                  <span className="font-medium">{org.name}</span>
                </li>
              ))}
            </ul>
          ) : (
            <p className="text-[--color-fg-muted]">{m.orgDashboard_empty()}</p>
          )}
        </section>
      ) : (
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
      )}
    </main>
  );
}
