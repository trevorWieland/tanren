"use client";

import type { ReactNode } from "react";

import type { AttentionSpecView, ProjectView } from "@/app/lib/project-client";
import { ProjectCard } from "@/components/project/ProjectCard";
import * as m from "@/i18n/paraglide/messages";

export interface ProjectListProps {
  projects: ProjectView[] | null;
  activeProject: ProjectView | null;
  onSwitch: (id: string) => void;
  switching: boolean;
  error: string | null;
  expandedSpec: AttentionSpecView | null;
  onExpandSpec: (spec: AttentionSpecView | null) => void;
}

export function ProjectList({
  projects,
  activeProject,
  onSwitch,
  switching,
  error,
  expandedSpec,
  onExpandSpec,
}: ProjectListProps): ReactNode {
  return (
    <div className="flex flex-col gap-3">
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

      {projects?.map((project) => (
        <ProjectCard
          key={project.id}
          project={project}
          isActive={activeProject?.id === project.id}
          onSwitch={onSwitch}
          switching={switching}
          expandedSpec={expandedSpec}
          onExpandSpec={onExpandSpec}
        />
      ))}
    </div>
  );
}
