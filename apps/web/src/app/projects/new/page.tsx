"use client";

import { useState } from "react";
import type { ReactNode } from "react";

import type { ProjectView } from "@/app/lib/project-client";
import { ConnectProjectForm } from "@/components/project/ConnectProjectForm";
import { CreateProjectForm } from "@/components/project/CreateProjectForm";
import { ActiveProjectSummary } from "@/components/project/ActiveProjectSummary";
import * as m from "@/i18n/paraglide/messages";

type Tab = "connect" | "create";

export default function NewProjectPage(): ReactNode {
  const [tab, setTab] = useState<Tab>("connect");
  const [activeProject, setActiveProject] = useState<ProjectView | null>(null);

  if (activeProject !== null) {
    return (
      <main className="flex min-h-screen flex-col items-center justify-center gap-6 p-8">
        <h1 className="text-2xl font-semibold">{m.projectActive_title()}</h1>
        <ActiveProjectSummary project={activeProject} />
      </main>
    );
  }

  return (
    <main className="flex min-h-screen flex-col items-center justify-center gap-6 p-8">
      <h1 className="text-2xl font-semibold">{m.projectNew_title()}</h1>
      <div className="flex gap-4">
        <button
          type="button"
          onClick={() => {
            setTab("connect");
          }}
          className={`rounded-md px-4 py-2 text-sm font-medium ${
            tab === "connect"
              ? "bg-[--color-accent] text-[--color-accent-fg]"
              : "text-[--color-fg-muted]"
          }`}
        >
          {m.projectNew_connectSubtitle()}
        </button>
        <button
          type="button"
          onClick={() => {
            setTab("create");
          }}
          className={`rounded-md px-4 py-2 text-sm font-medium ${
            tab === "create"
              ? "bg-[--color-accent] text-[--color-accent-fg]"
              : "text-[--color-fg-muted]"
          }`}
        >
          {m.projectNew_createSubtitle()}
        </button>
      </div>
      {tab === "connect" ? (
        <ConnectProjectForm onSuccess={setActiveProject} />
      ) : (
        <CreateProjectForm onSuccess={setActiveProject} />
      )}
    </main>
  );
}
