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
import type {
  ProjectCollectionFreshnessView,
  ProjectPaginationView,
  ProjectView,
} from "@/app/lib/contracts";
import { ConnectRepositoryForm } from "@/components/project/ConnectRepositoryForm";
import { ProjectList } from "@/components/project/ProjectList";
import * as m from "@/i18n/paraglide/messages";

export default function ProjectsPage(): ReactNode {
  const [projects, setProjects] = useState<ProjectView[]>([]);
  const [pagination, setPagination] = useState<ProjectPaginationView | null>(
    null,
  );
  const [freshness, setFreshness] =
    useState<ProjectCollectionFreshnessView | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const visible = await listVisibleProjects();
        if (!cancelled) {
          setProjects(normalizeActiveProjects(visible.projects));
          setPagination(visible.pagination);
          setFreshness(visible.freshness);
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
        setErrorMessage(m.projects_connect_failed());
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  return (
    <main className="flex min-h-screen flex-col items-center gap-6 p-8">
      <h1 className="text-2xl font-semibold">{m.projects_title()}</h1>
      <p className="m-0 max-w-2xl text-[--color-fg-muted]">
        {m.projects_connect_subtitle()}
      </p>
      <ConnectRepositoryForm
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
      <div
        className="w-full max-w-2xl"
        data-page-size={pagination?.page_size}
        data-has-more={pagination?.has_more}
        data-freshness-as-of={freshness?.as_of ?? undefined}
      >
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
