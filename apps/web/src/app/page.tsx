import { useQuery } from "@tanstack/react-query";
import type { ReactNode } from "react";

import * as m from "@/i18n/paraglide/messages";
import { listOrganizations } from "@/app/lib/account-client";
import { OrganizationSwitcher } from "@/components/account/OrganizationSwitcher";

interface HealthReport {
  status: string;
  version: string;
  contract_version: number;
}

const API_URL = import.meta.env.VITE_API_URL ?? "http://localhost:8080";

async function fetchHealth(): Promise<HealthReport> {
  const response = await fetch(`${API_URL}/health`, { credentials: "include" });
  if (!response.ok) {
    throw new Error(`HTTP ${response.status}`);
  }
  return (await response.json()) as HealthReport;
}

export default function Home(): ReactNode {
  const healthQuery = useQuery({
    queryKey: ["health"],
    queryFn: fetchHealth,
  });

  const orgQuery = useQuery({
    queryKey: ["organizations"],
    queryFn: listOrganizations,
  });

  const report = healthQuery.data ?? null;
  const healthError = healthQuery.error;
  const orgState = orgQuery.data ?? null;

  return (
    <main className="flex min-h-screen flex-col items-center justify-center gap-6 p-8">
      <h1 className="text-3xl font-semibold">{m.app_title()}</h1>
      <p className="text-[--color-fg-muted]">{m.app_placeholder()}</p>

      {orgState !== null ? (
        <div className="flex w-full max-w-lg flex-col gap-6">
          <OrganizationSwitcher
            data={orgState}
            onSwitched={() => {
              orgQuery.refetch();
            }}
          />
        </div>
      ) : (
        <section className="min-w-[20rem] rounded-md border border-[--color-border] bg-[--color-bg-surface] px-6 py-4 font-mono">
          {report !== null ? (
            <pre className="m-0">{JSON.stringify(report, null, 2)}</pre>
          ) : healthError !== null ? (
            <span className="text-[--color-error]">
              {m.app_health_unreachable()}:{" "}
              {healthError instanceof Error
                ? healthError.message
                : String(healthError)}
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
