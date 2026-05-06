"use client";

import type { ReactNode } from "react";

import type { ProjectView } from "@/app/lib/project-client";
import * as m from "@/i18n/paraglide/messages";

export interface ProjectSwitcherProps {
  projects: ProjectView[] | null;
  activeProject: ProjectView | null;
  onSwitch: (id: string) => void;
  switching: boolean;
}

export function ProjectSwitcher({
  projects,
  activeProject,
  onSwitch,
  switching,
}: ProjectSwitcherProps): ReactNode {
  return (
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
          onSwitch(event.target.value);
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
  );
}
