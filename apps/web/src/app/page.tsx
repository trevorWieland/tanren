"use client";

import { useState } from "react";
import type { ReactNode } from "react";

import type { AttentionSpecView } from "@/app/lib/project-client";
import {
  useActiveProject,
  useActiveProjectViews,
  useProjectList,
  useSignOutMutation,
  useSwitchProjectMutation,
} from "@/app/lib/project-queries";
import { ProjectList } from "@/components/project/ProjectList";
import { ProjectSwitcher } from "@/components/project/ProjectSwitcher";
import { ScopedViewsSummary } from "@/components/project/ScopedViewsSummary";
import * as m from "@/i18n/paraglide/messages";

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

  function handleSwitch(id: string): void {
    setExpandedSpec(null);
    switchMutation.mutate(id);
  }

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
        <ProjectSwitcher
          projects={projects}
          activeProject={activeProject}
          onSwitch={handleSwitch}
          switching={switching}
        />

        <ProjectList
          projects={projects}
          activeProject={activeProject}
          onSwitch={handleSwitch}
          switching={switching}
          error={error}
          expandedSpec={expandedSpec}
          onExpandSpec={setExpandedSpec}
        />

        {activeProject !== null && scopedViews != null && (
          <ScopedViewsSummary
            projectName={activeProject.name}
            scopedViews={scopedViews}
          />
        )}
      </div>
    </div>
  );
}
