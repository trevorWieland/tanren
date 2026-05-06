"use client";

import { useState } from "react";
import type { ReactNode } from "react";

import {
  type AttentionSpecView,
  type ProjectView,
} from "@/app/lib/project-client";
import {
  useActiveProject,
  useActiveProjectViews,
  useProjectList,
  useSignOutMutation,
  useSwitchProjectMutation,
} from "@/app/lib/project-queries";
import * as m from "@/i18n/paraglide/messages";

const stateLabel: Record<string, string> = {
  active: m.projects_stateActive(),
  paused: m.projects_statePaused(),
  completed: m.projects_stateCompleted(),
  archived: m.projects_stateArchived(),
};

export default function Home(): ReactNode {
  const projectListQuery = useProjectList();
  const activeProject = useActiveProject();
  const { data: scopedViews } = useActiveProjectViews();
  const switchMutation = useSwitchProjectMutation();
  const signOutMutation = useSignOutMutation();
  const [expandedSpec, setExpandedSpec] = useState<AttentionSpecView | null>(
    null,
  );

  const projects = projectListQuery.data ?? null;
  const authError =
    projectListQuery.authError ||
    signOutMutation.isSuccess ||
    signOutMutation.isError;
  const queryError =
    !projectListQuery.authError && projectListQuery.error !== null
      ? projectListQuery.error instanceof Error
        ? projectListQuery.error.message
        : String(projectListQuery.error)
      : null;
  const mutationError =
    switchMutation.error instanceof Error ? switchMutation.error.message : null;
  const error = mutationError ?? queryError;
  const switching = switchMutation.isPending;

  if (authError) {
    return (
      <main className="flex min-h-screen flex-col items-center justify-center gap-4 p-6">
        <h1 className="text-2xl font-semibold">{m.app_title()}</h1>
        <p>{m.projects_signInPrompt()}</p>
        <a
          href="/sign-in"
          className="rounded-md border border-[--color-border] bg-[--color-accent] px-4 py-2 text-base font-medium text-[--color-accent-fg] hover:bg-[--color-accent-hover]"
        >
          {m.projects_signIn()}
        </a>
      </main>
    );
  }

  return (
    <div className="min-h-screen">
      <header className="flex flex-wrap items-center justify-between gap-2 border-b border-[--color-border] px-4 py-3">
        <h1 className="text-lg font-semibold">{m.app_title()}</h1>
        <button
          type="button"
          onClick={() => {
            signOutMutation.mutate();
          }}
          className="rounded-md border border-[--color-border] px-3 py-1 text-sm text-[--color-fg-default] hover:bg-[--color-bg-elevated]"
        >
          {m.projects_signOut()}
        </button>
      </header>

      <div className="mx-auto max-w-3xl px-4 py-4">
        <div className="mb-4 flex flex-col gap-2 sm:flex-row sm:items-center">
          <label
            htmlFor="active-project-select"
            className="text-sm font-medium text-[--color-fg-muted]"
          >
            {m.projects_activeProject()}:
          </label>
          <select
            id="active-project-select"
            value={activeProject?.id ?? ""}
            onChange={(event) => {
              setExpandedSpec(null);
              switchMutation.mutate(event.target.value);
            }}
            disabled={switching || projects === null}
            className="min-w-0 flex-1 rounded-md border border-[--color-border] bg-[--color-bg-surface] px-3 py-2 text-sm text-[--color-fg-default] focus:outline-none focus:ring-2 focus:ring-[--color-accent]"
          >
            <option value="">
              {switching ? m.projects_switching() : "\u2014"}
            </option>
            {projects?.map((project) => (
              <option key={project.id} value={project.id}>
                {project.name}
              </option>
            ))}
          </select>
        </div>

        <h2 className="mb-3 text-xl font-semibold">{m.projects_title()}</h2>

        {projects === null && error === null && (
          <p className="text-[--color-fg-muted]">{m.projects_loading()}</p>
        )}
        {error !== null && (
          <p className="text-[--color-error]" role="alert">
            {m.projects_error()} {error}
          </p>
        )}
        {projects !== null && projects.length === 0 && (
          <p className="text-[--color-fg-muted]">{m.projects_empty()}</p>
        )}

        <div className="flex flex-col gap-3">
          {projects?.map((project) => (
            <ProjectCard
              key={project.id}
              project={project}
              isActive={activeProject?.id === project.id}
              onSwitch={(id) => {
                setExpandedSpec(null);
                switchMutation.mutate(id);
              }}
              switching={switching}
              expandedSpec={expandedSpec}
              onExpandSpec={setExpandedSpec}
            />
          ))}
        </div>

        {activeProject !== null && scopedViews != null && (
          <section
            aria-label={m.projects_scopedViews()}
            className="mt-6 rounded-md border border-[--color-border] bg-[--color-bg-surface] p-4"
          >
            <h3 className="mb-2 text-base font-semibold">
              {m.projects_scopedViews()} \u2014 {activeProject.name}
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
        )}
      </div>
    </div>
  );
}

interface ProjectCardProps {
  project: ProjectView;
  isActive: boolean;
  onSwitch: (id: string) => void;
  switching: boolean;
  expandedSpec: AttentionSpecView | null;
  onExpandSpec: (spec: AttentionSpecView | null) => void;
}

function ProjectCard({
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
