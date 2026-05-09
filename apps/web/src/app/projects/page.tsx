"use client";

import Link from "next/link";
import { useState } from "react";
import type { ReactNode } from "react";

import type { ProjectView } from "@/app/lib/project-client";
import { ConnectRepositoryForm } from "@/components/project/ConnectRepositoryForm";
import { ProjectList } from "@/components/project/ProjectList";
import * as m from "@/i18n/paraglide/messages";

function mergeProject(
  projects: ProjectView[],
  next: ProjectView,
): ProjectView[] {
  const withoutCurrent = projects.filter((project) => project.id !== next.id);
  const normalized = next.selection.is_active
    ? withoutCurrent.map((project) => ({
        ...project,
        selection: {
          ...project.selection,
          is_active: false,
          selected_at: null,
        },
      }))
    : withoutCurrent;
  return [next, ...normalized];
}

export default function ProjectsPage(): ReactNode {
  const [projects, setProjects] = useState<ProjectView[]>([]);

  return (
    <main className="flex min-h-screen flex-col items-center gap-6 p-8">
      <h1 className="text-2xl font-semibold">{m.projects_title()}</h1>
      <p className="m-0 max-w-2xl text-[--color-fg-muted]">
        {m.projects_connect_subtitle()}
      </p>
      <ConnectRepositoryForm
        onSuccess={(result) => {
          setProjects((current) => mergeProject(current, result.project));
        }}
      />
      <div className="w-full max-w-2xl">
        <h2 className="mb-3 text-lg font-medium">{m.projects_list_title()}</h2>
        <ProjectList projects={projects} />
      </div>
      <Link
        href="/projects/new"
        className="text-sm text-[--color-accent] underline underline-offset-2"
      >
        {m.projects_create_link()}
      </Link>
    </main>
  );
}
