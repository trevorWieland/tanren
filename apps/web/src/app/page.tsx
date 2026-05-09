"use client";

import Link from "next/link";
import { useEffect, useState } from "react";
import type { ReactNode } from "react";

import { canAccessMyPermissions } from "@/app/lib/account-client";
import * as m from "@/i18n/paraglide/messages";

interface HealthReport {
  status: string;
  version: string;
  contract_version: number;
}

const API_URL = process.env["NEXT_PUBLIC_API_URL"] ?? "http://localhost:8080";

export default function Home(): ReactNode {
  const [report, setReport] = useState<HealthReport | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [showMyPermissions, setShowMyPermissions] = useState(false);

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

  useEffect(() => {
    let cancelled = false;
    canAccessMyPermissions()
      .then((allowed) => {
        if (!cancelled) {
          setShowMyPermissions(allowed);
        }
      })
      .catch(() => {
        if (!cancelled) {
          setShowMyPermissions(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, []);

  return (
    <main className="mx-auto flex min-h-screen w-full max-w-4xl flex-col gap-6 px-4 py-8 sm:px-6">
      <header className="space-y-2">
        <h1 className="text-3xl font-semibold">{m.app_title()}</h1>
        <p className="text-[--color-fg-muted]">{m.app_placeholder()}</p>
      </header>
      <nav className="flex flex-wrap gap-3">
        <Link
          href="/sign-up"
          className="rounded-md border border-[--color-border] bg-[--color-bg-surface] px-4 py-2 text-sm hover:bg-[--color-bg-elevated]"
        >
          {m.app_nav_signUp()}
        </Link>
        <Link
          href="/sign-in"
          className="rounded-md border border-[--color-border] bg-[--color-bg-surface] px-4 py-2 text-sm hover:bg-[--color-bg-elevated]"
        >
          {m.app_nav_signIn()}
        </Link>
        {showMyPermissions ? (
          <Link
            href="/my-permissions"
            className="rounded-md border border-[--color-border] bg-[--color-bg-surface] px-4 py-2 text-sm hover:bg-[--color-bg-elevated]"
          >
            {m.app_nav_myPermissions()}
          </Link>
        ) : null}
      </nav>
      <section className="w-full rounded-md border border-[--color-border] bg-[--color-bg-surface] px-4 py-4 font-mono sm:px-6">
        {report !== null ? (
          <pre className="m-0 overflow-x-auto">
            {JSON.stringify(report, null, 2)}
          </pre>
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
