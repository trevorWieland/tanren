"use client";

import type { ReactNode } from "react";

import type { AttentionSpecView, ProjectView } from "@/app/lib/project-client";
import * as m from "@/i18n/paraglide/messages";

const stateLabel: Record<string, string> = {
  active: m.projects_stateActive(),
  paused: m.projects_statePaused(),
  completed: m.projects_stateCompleted(),
  archived: m.projects_stateArchived(),
};

export interface ProjectCardProps {
  project: ProjectView;
  isActive: boolean;
  onSwitch: (id: string) => void;
  switching: boolean;
  expandedSpec: AttentionSpecView | null;
  onExpandSpec: (spec: AttentionSpecView | null) => void;
}

export function ProjectCard({
  project,
  isActive,
  onSwitch,
  switching,
  expandedSpec,
  onExpandSpec,
}: ProjectCardProps): ReactNode {
  const label = stateLabel[project.state] ?? project.state;

  return (
    <article
      className={[
        "rounded-md border p-4",
        isActive
          ? "border-[--color-accent] bg-[--color-bg-elevated]"
          : "border-[--color-border] bg-[--color-bg-surface]",
      ].join(" ")}
    >
      <div className="flex flex-wrap items-center justify-between gap-2">
        <div className="flex flex-wrap items-center gap-2">
          <span className="text-base font-medium text-[--color-fg-default]">
            {project.name}
          </span>
          <span className="rounded-sm bg-[--color-bg-canvas] px-2 py-0.5 text-xs text-[--color-fg-muted]">
            {label}
          </span>
          {project.needs_attention && (
            <span className="rounded-sm bg-[--color-warning] px-2 py-0.5 text-xs font-semibold text-[--color-bg-canvas]">
              {m.projects_attention()}
            </span>
          )}
        </div>
        {!isActive && (
          <button
            type="button"
            onClick={() => {
              onSwitch(project.id);
            }}
            disabled={switching}
            className="shrink-0 rounded-md border border-[--color-border] bg-[--color-accent] px-3 py-1 text-sm font-medium text-[--color-accent-fg] hover:bg-[--color-accent-hover] disabled:opacity-60"
          >
            {switching ? m.projects_switching() : m.projects_switch()}
          </button>
        )}
      </div>

      {project.attention_specs.length > 0 && (
        <ul className="mt-2 list-none space-y-1 p-0">
          {project.attention_specs.map((spec) => (
            <li key={spec.id}>
              <button
                type="button"
                onClick={() => {
                  onExpandSpec(expandedSpec?.id === spec.id ? null : spec);
                }}
                className="text-left text-sm text-[--color-warning] underline decoration-dotted hover:text-[--color-fg-default]"
              >
                {spec.name}
              </button>
            </li>
          ))}
        </ul>
      )}

      {project.attention_specs.length === 0 && (
        <p className="mt-1 text-xs text-[--color-fg-muted]">
          {m.projects_noAttention()}
        </p>
      )}

      {expandedSpec !== null &&
        project.attention_specs.some((spec) => spec.id === expandedSpec.id) && (
          <div
            role="region"
            aria-label={m.projects_specDetail()}
            className="mt-2 rounded-sm border border-[--color-border] bg-[--color-bg-canvas] p-3 text-sm"
          >
            <p className="m-0 font-medium text-[--color-fg-default]">
              {expandedSpec.name}
            </p>
            <p className="m-0 mt-1 text-[--color-fg-muted]">
              {expandedSpec.reason}
            </p>
          </div>
        )}
    </article>
  );
}
