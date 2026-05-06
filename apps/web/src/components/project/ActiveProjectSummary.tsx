"use client";

import type { ReactNode } from "react";

import type { ProjectView } from "@/app/lib/project-client";
import * as m from "@/i18n/paraglide/messages";

export interface ActiveProjectSummaryProps {
  project: ProjectView;
}

export function ActiveProjectSummary({
  project,
}: ActiveProjectSummaryProps): ReactNode {
  return (
    <section
      aria-label={m.projectActive_title()}
      className="w-full max-w-md rounded-md border border-[--color-border] bg-[--color-bg-surface] px-6 py-4"
    >
      <h2 className="m-0 mb-3 text-lg font-semibold">{project.name}</h2>
      <dl className="m-0 grid grid-cols-[auto_1fr] gap-x-4 gap-y-2 text-sm">
        <dt className="text-[--color-fg-muted]">
          {m.projectActive_repository()}
        </dt>
        <dd className="m-0 font-mono">{project.repository.url}</dd>
        <dt className="text-[--color-fg-muted]">{m.projectActive_specs()}</dt>
        <dd className="m-0">{project.content_counts.specs}</dd>
        <dt className="text-[--color-fg-muted]">
          {m.projectActive_milestones()}
        </dt>
        <dd className="m-0">{project.content_counts.milestones}</dd>
        <dt className="text-[--color-fg-muted]">
          {m.projectActive_initiatives()}
        </dt>
        <dd className="m-0">{project.content_counts.initiatives}</dd>
      </dl>
    </section>
  );
}
