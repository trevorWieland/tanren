"use client";

import { useId, useState, useTransition } from "react";
import type { FormEvent, ReactNode } from "react";
import * as v from "valibot";

import {
  UninstallRequestError,
  describeUninstallFailure,
  previewUninstall,
  applyUninstall,
  type UninstallPreviewResponse,
  type UninstallApplyResponse,
  type PreservedFile,
} from "@/app/lib/project-uninstall-client";
import * as m from "@/i18n/paraglide/messages";

const RepoPathInput = v.object({
  repo_path: v.pipe(v.string(), v.trim(), v.minLength(1)),
});

type Phase =
  | "idle"
  | "previewing"
  | "preview"
  | "applying"
  | "applied"
  | "error";

export interface UninstallPanelProps {
  repoPath?: string | undefined;
  onApplied?: ((response: UninstallApplyResponse) => void) | undefined;
}

const inputClass =
  "w-full rounded-md border border-[--color-border] bg-[--color-bg-surface] px-3 py-2 font-mono text-sm text-[--color-fg-default] focus:outline-none focus:ring-2 focus:ring-[--color-accent]";

const buttonPrimary =
  "rounded-md border border-[--color-border] bg-[--color-accent] px-4 py-2 text-base font-medium text-[--color-accent-fg] transition-colors hover:bg-[--color-accent-hover] disabled:opacity-60";

const buttonDestructive =
  "rounded-md border border-[--color-error] bg-[--color-error] px-4 py-2 text-base font-medium text-[--color-fg-inverse] transition-colors hover:opacity-90 disabled:opacity-60";

function preserveReasonLabel(reason: PreservedFile["reason"]): string {
  switch (reason) {
    case "UserOwned":
      return m.uninstall_reasonUserOwned();
    case "ModifiedSinceInstall":
      return m.uninstall_reasonModified();
    case "AlreadyRemoved":
      return m.uninstall_reasonAlreadyRemoved();
  }
}

export function UninstallPanel({
  repoPath: initialRepoPath,
  onApplied,
}: UninstallPanelProps): ReactNode {
  const baseId = useId();
  const repoPathId = `${baseId}-repo-path`;
  const confirmId = `${baseId}-confirm`;
  const errorId = `${baseId}-error`;

  const [repoPath, setRepoPath] = useState(initialRepoPath ?? "");
  const [phase, setPhase] = useState<Phase>("idle");
  const [previewData, setPreviewData] =
    useState<UninstallPreviewResponse | null>(null);
  const [applyData, setApplyData] = useState<UninstallApplyResponse | null>(
    null,
  );
  const [confirmed, setConfirmed] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [, startTransition] = useTransition();

  function handlePreview(event: FormEvent<HTMLFormElement>): void {
    event.preventDefault();
    setErrorMessage(null);
    setPreviewData(null);
    setApplyData(null);
    setConfirmed(false);

    const parsed = v.safeParse(RepoPathInput, { repo_path: repoPath });
    if (!parsed.success) {
      setErrorMessage(m.uninstall_repoPathRequired());
      setPhase("error");
      return;
    }

    startTransition(async () => {
      setPhase("previewing");
      try {
        const data = await previewUninstall(parsed.output.repo_path);
        setPreviewData(data);
        setPhase("preview");
      } catch (cause: unknown) {
        if (cause instanceof UninstallRequestError) {
          setErrorMessage(describeUninstallFailure(cause.failure));
        } else if (cause instanceof Error) {
          setErrorMessage(cause.message);
        } else {
          setErrorMessage(m.uninstall_failed());
        }
        setPhase("error");
      }
    });
  }

  function handleApply(): void {
    if (!confirmed || previewData === null) {
      return;
    }
    setErrorMessage(null);

    startTransition(async () => {
      setPhase("applying");
      try {
        const data = await applyUninstall(repoPath);
        setApplyData(data);
        setPhase("applied");
        onApplied?.(data);
      } catch (cause: unknown) {
        if (cause instanceof UninstallRequestError) {
          setErrorMessage(describeUninstallFailure(cause.failure));
        } else if (cause instanceof Error) {
          setErrorMessage(cause.message);
        } else {
          setErrorMessage(m.uninstall_failed());
        }
        setPhase("error");
      }
    });
  }

  const showForm = phase === "idle" || phase === "error";
  const errorActive = errorMessage !== null;

  return (
    <div className="flex w-full max-w-2xl flex-col gap-6">
      {showForm && (
        <form
          onSubmit={handlePreview}
          noValidate
          aria-label={m.uninstall_formLabel()}
          className="flex flex-col gap-4"
        >
          <div className="flex flex-col gap-1">
            <label htmlFor={repoPathId} className="text-sm font-medium">
              {m.uninstall_repoPathLabel()}
            </label>
            <input
              id={repoPathId}
              name="repo_path"
              type="text"
              value={repoPath}
              onChange={(e) => {
                setRepoPath(e.target.value);
              }}
              aria-describedby={errorActive ? errorId : undefined}
              aria-invalid={errorActive ? true : undefined}
              className={inputClass}
              placeholder="/path/to/repository"
            />
          </div>
          <button type="submit" className={buttonPrimary}>
            {m.uninstall_previewButton()}
          </button>
        </form>
      )}

      {phase === "previewing" && (
        <p className="text-[--color-fg-muted]">
          {m.uninstall_previewLoading()}
        </p>
      )}

      {phase === "preview" && previewData !== null && (
        <div className="flex flex-col gap-5">
          <section className="flex flex-col gap-2">
            <h2 className="text-lg font-semibold">
              {m.uninstall_filesToRemove()}
            </h2>
            {previewData.preview.to_remove.length === 0 ? (
              <p className="text-[--color-fg-muted]">
                {m.uninstall_noFilesToRemove()}
              </p>
            ) : (
              <ul className="list-inside list-disc font-mono text-sm">
                {previewData.preview.to_remove.map((path) => (
                  <li key={path}>{path}</li>
                ))}
              </ul>
            )}
          </section>

          <section className="flex flex-col gap-2">
            <h2 className="text-lg font-semibold">
              {m.uninstall_filesPreserved()}
            </h2>
            {previewData.preview.preserved.length === 0 ? (
              <p className="text-[--color-fg-muted]">
                {m.uninstall_noFilesPreserved()}
              </p>
            ) : (
              <ul className="list-inside list-disc font-mono text-sm">
                {previewData.preview.preserved.map((entry) => (
                  <li key={entry.path}>
                    {entry.path}{" "}
                    <span className="font-sans text-xs text-[--color-fg-muted]">
                      ({preserveReasonLabel(entry.reason)})
                    </span>
                  </li>
                ))}
              </ul>
            )}
          </section>

          <p className="rounded-md border border-[--color-border] bg-[--color-bg-surface] px-4 py-3 text-sm text-[--color-fg-muted]">
            {m.uninstall_hostedNote()}
          </p>

          <div className="flex items-center gap-2">
            <input
              id={confirmId}
              type="checkbox"
              checked={confirmed}
              onChange={(e) => {
                setConfirmed(e.target.checked);
              }}
              className="h-4 w-4 accent-[--color-accent]"
            />
            <label htmlFor={confirmId} className="text-sm">
              {m.uninstall_confirmLabel()}
            </label>
          </div>

          <button
            type="button"
            disabled={!confirmed}
            onClick={handleApply}
            className={buttonDestructive}
          >
            {m.uninstall_applyButton()}
          </button>
        </div>
      )}

      {phase === "applying" && (
        <p className="text-[--color-fg-muted]">{m.uninstall_applying()}</p>
      )}

      {phase === "applied" && applyData !== null && (
        <div className="flex flex-col gap-4">
          <p className="text-[--color-success]">{m.uninstall_success()}</p>
          {applyData.result.removed.length > 0 && (
            <section className="flex flex-col gap-2">
              <h2 className="text-lg font-semibold">
                {m.uninstall_removedHeading()}
              </h2>
              <ul className="list-inside list-disc font-mono text-sm">
                {applyData.result.removed.map((path) => (
                  <li key={path}>{path}</li>
                ))}
              </ul>
            </section>
          )}
          {applyData.result.preserved.length > 0 && (
            <section className="flex flex-col gap-2">
              <h2 className="text-lg font-semibold">
                {m.uninstall_preservedHeading()}
              </h2>
              <ul className="list-inside list-disc font-mono text-sm">
                {applyData.result.preserved.map((entry) => (
                  <li key={entry.path}>
                    {entry.path}{" "}
                    <span className="font-sans text-xs text-[--color-fg-muted]">
                      ({preserveReasonLabel(entry.reason)})
                    </span>
                  </li>
                ))}
              </ul>
            </section>
          )}
          {!applyData.result.manifest_removed && (
            <p className="text-[--color-fg-muted] text-sm">
              {m.uninstall_manifestNotRemoved()}
            </p>
          )}
          <p className="rounded-md border border-[--color-border] bg-[--color-bg-surface] px-4 py-3 text-sm text-[--color-fg-muted]">
            {m.uninstall_hostedNote()}
          </p>
        </div>
      )}

      {errorActive && (
        <p
          id={errorId}
          role="alert"
          aria-live="polite"
          className="m-0 text-[--color-error]"
        >
          {errorMessage}
        </p>
      )}
    </div>
  );
}
