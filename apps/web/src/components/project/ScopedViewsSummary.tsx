"use client";

import type { ReactNode } from "react";

import type { ScopedViewsResponse } from "@/app/lib/project-client";
import * as m from "@/i18n/paraglide/messages";

export interface ScopedViewsSummaryProps {
  projectName: string;
  scopedViews: ScopedViewsResponse;
}

export function ScopedViewsSummary({
  projectName,
  scopedViews,
}: ScopedViewsSummaryProps): ReactNode {
  return (
    <section
      aria-label={m.projects_scopedViews()}
      className="mt-6 rounded-md border border-[--color-border] bg-[--color-bg-surface] p-4"
    >
      <h3 className="mb-2 text-base font-semibold">
        {m.projects_scopedViews()} — {projectName}
      </h3>
      <div className="flex flex-wrap gap-4 text-sm">
        <span>
          {m.projects_specs()}: {scopedViews.specs.length}
        </span>
        <span>
          {m.projects_loops()}: {scopedViews.loops.length}
        </span>
        <span>
          {m.projects_milestones()}: {scopedViews.milestones.length}
        </span>
      </div>
    </section>
  );
}
