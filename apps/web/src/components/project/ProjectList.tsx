import type { ReactNode } from "react";

import type { ProjectView } from "@/app/lib/project-client";
import * as m from "@/i18n/paraglide/messages";

export interface ProjectListProps {
  projects: ProjectView[];
}

function projectStatusLabel(project: ProjectView): string {
  return project.selection.is_active
    ? m.projects_list_statusActive()
    : m.projects_list_statusSelectable();
}

export function ProjectList({ projects }: ProjectListProps): ReactNode {
  if (projects.length === 0) {
    return (
      <p className="m-0 text-[--color-fg-muted]">{m.projects_list_empty()}</p>
    );
  }

  return (
    <ul className="m-0 flex w-full max-w-2xl list-none flex-col gap-3 p-0">
      {projects.map((project) => (
        <li
          key={project.id}
          className="rounded-md border border-[--color-border] bg-[--color-bg-surface] px-4 py-3"
        >
          <div className="flex items-center justify-between gap-2">
            <code className="text-sm text-[--color-fg-default]">
              {project.repository.repository}
            </code>
            <span
              className={
                project.selection.is_active
                  ? "rounded border border-[--color-accent] px-2 py-0.5 text-xs font-medium text-[--color-accent]"
                  : "rounded border border-[--color-border] px-2 py-0.5 text-xs font-medium text-[--color-fg-muted]"
              }
            >
              {projectStatusLabel(project)}
            </span>
          </div>
          <div className="mt-2 text-sm text-[--color-fg-muted]">
            {m.projects_list_countsPrefix()} specs={project.counts.specs}{" "}
            milestones={project.counts.milestones} initiatives=
            {project.counts.initiatives}
          </div>
        </li>
      ))}
    </ul>
  );
}
