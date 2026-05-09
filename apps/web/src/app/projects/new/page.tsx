"use client";

import Link from "next/link";
import { useEffect, useState } from "react";
import type { ReactNode } from "react";

import {
  ProjectRequestError,
  listVisibleProjects,
} from "@/app/lib/project-client";
import {
  mergeProjectIntoVisibleProjects,
  normalizeActiveProjects,
} from "@/app/lib/project-merge";
import type { ProjectView } from "@/app/lib/contracts";
import { CreateProjectForm } from "@/components/project/CreateProjectForm";
import { ProjectList } from "@/components/project/ProjectList";
import * as m from "@/i18n/paraglide/messages";

export default function NewProjectPage(): ReactNode {
  const [projects, setProjects] = useState<ProjectView[]>([]);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const visible = await listVisibleProjects();
        if (!cancelled) {
          setProjects(normalizeActiveProjects(visible.projects));
          setErrorMessage(null);
        }
      } catch (cause: unknown) {
        if (cancelled) {
          return;
        }
        if (cause instanceof ProjectRequestError) {
          setErrorMessage(cause.message);
          return;
        }
        if (cause instanceof Error) {
          setErrorMessage(cause.message);
          return;
        }
        setErrorMessage(m.projects_create_failed());
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  return (
    <main className="flex min-h-screen flex-col items-center gap-6 p-8">
      <h1 className="text-2xl font-semibold">{m.projects_create_title()}</h1>
      <p className="m-0 max-w-2xl text-[--color-fg-muted]">
        {m.projects_create_subtitle()}
      </p>
      <CreateProjectForm
        onSuccess={(result) => {
          setProjects((current) =>
            mergeProjectIntoVisibleProjects(current, result.project),
          );
        }}
      />
      {errorMessage !== null && (
        <p role="alert" className="m-0 text-[--color-error]">
          {errorMessage}
        </p>
      )}
      <div className="w-full max-w-2xl">
        <h2 className="mb-3 text-lg font-medium">{m.projects_list_title()}</h2>
        <ProjectList projects={projects} />
      </div>
      <Link
        href="/projects"
        className="text-sm text-[--color-accent] underline underline-offset-2"
      >
        {m.projects_connect_link()}
      </Link>
    </main>
  );
}
