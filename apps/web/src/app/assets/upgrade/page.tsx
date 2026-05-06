"use client";

import { useId, useState, useTransition } from "react";
import type { FormEvent, ReactNode } from "react";
import * as v from "valibot";

import {
  AssetRequestError,
  describeAssetFailure,
  previewUpgrade,
  applyUpgrade,
  type AssetAction,
  type MigrationConcern,
  type UpgradePreviewResponse,
} from "@/app/lib/assets-client";
import * as m from "@/i18n/paraglide/messages";

const RootInput = v.object({
  root: v.pipe(v.string(), v.trim(), v.minLength(1)),
});

const inputClass =
  "rounded-md border border-[--color-border] bg-[--color-bg-surface] px-3 py-2 text-base text-[--color-fg-default] focus:outline-none focus:ring-2 focus:ring-[--color-accent]";

const buttonClass =
  "rounded-md border border-[--color-border] bg-[--color-accent] px-4 py-2 text-base font-medium text-[--color-accent-fg] transition-colors hover:bg-[--color-accent-hover] disabled:opacity-60";

const dangerButtonClass =
  "rounded-md border border-[--color-border] bg-[--color-error] px-4 py-2 text-base font-medium text-[--color-fg-inverse] transition-colors hover:opacity-90 disabled:opacity-60";

type Step = "input" | "preview" | "applied";

export default function UpgradePage(): ReactNode {
  const baseId = useId();
  const rootId = `${baseId}-root`;
  const errorId = `${baseId}-error`;

  const [root, setRoot] = useState("");
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [pending, startTransition] = useTransition();
  const [step, setStep] = useState<Step>("input");
  const [preview, setPreview] = useState<UpgradePreviewResponse | null>(null);

  function onSubmitPreview(event: FormEvent<HTMLFormElement>): void {
    event.preventDefault();
    setErrorMessage(null);
    const parsed = v.safeParse(RootInput, { root });
    if (!parsed.success) {
      setErrorMessage(m.upgrade_rootRequired());
      return;
    }
    startTransition(async () => {
      try {
        const result = await previewUpgrade(parsed.output.root);
        setPreview(result);
        setStep("preview");
      } catch (cause: unknown) {
        handleError(cause);
      }
    });
  }

  function onSubmitApply(): void {
    setErrorMessage(null);
    startTransition(async () => {
      try {
        const result = await applyUpgrade(root);
        setPreview(result);
        setStep("applied");
      } catch (cause: unknown) {
        handleError(cause);
      }
    });
  }

  function handleError(cause: unknown): void {
    if (cause instanceof AssetRequestError) {
      setErrorMessage(describeAssetFailure(cause.failure));
    } else if (cause instanceof Error) {
      setErrorMessage(cause.message);
    } else {
      setErrorMessage(m.upgrade_failed());
    }
  }

  const errorActive = errorMessage !== null;

  return (
    <main className="flex min-h-screen flex-col items-center justify-center gap-6 p-8">
      <h1 className="text-2xl font-semibold">{m.upgrade_title()}</h1>

      {step === "input" && (
        <form
          onSubmit={onSubmitPreview}
          noValidate
          aria-label={m.upgrade_formLabel()}
          className="flex w-full max-w-lg flex-col gap-4"
        >
          <div className="flex flex-col gap-1">
            <label htmlFor={rootId} className="text-sm font-medium">
              {m.upgrade_rootLabel()}
            </label>
            <input
              id={rootId}
              name="root"
              type="text"
              value={root}
              onChange={(event) => {
                setRoot(event.target.value);
              }}
              aria-describedby={errorActive ? errorId : undefined}
              aria-invalid={errorActive ? true : undefined}
              className={inputClass}
            />
          </div>
          <button type="submit" disabled={pending} className={buttonClass}>
            {pending ? m.upgrade_submitting() : m.upgrade_preview()}
          </button>
        </form>
      )}

      {step === "preview" && preview !== null && (
        <div className="flex w-full max-w-lg flex-col gap-4">
          <PreviewResults response={preview} />
          <div className="flex gap-3">
            <button
              type="button"
              disabled={pending}
              className={dangerButtonClass}
              onClick={onSubmitApply}
            >
              {pending ? m.upgrade_submitting() : m.upgrade_apply()}
            </button>
            <button
              type="button"
              className={buttonClass}
              onClick={() => {
                setStep("input");
                setPreview(null);
              }}
            >
              {m.upgrade_cancel()}
            </button>
          </div>
        </div>
      )}

      {step === "applied" && preview !== null && (
        <div className="flex w-full max-w-lg flex-col gap-4">
          <p className="text-[--color-success]">{m.upgrade_success()}</p>
          <PreviewResults response={preview} />
          <button
            type="button"
            className={buttonClass}
            onClick={() => {
              setStep("input");
              setPreview(null);
              setRoot("");
            }}
          >
            {m.upgrade_done()}
          </button>
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
    </main>
  );
}

function PreviewResults({
  response,
}: {
  response: UpgradePreviewResponse;
}): ReactNode {
  return (
    <section className="flex flex-col gap-3">
      <p className="text-sm text-[--color-fg-muted]">
        {m.upgrade_versionInfo()}: {response.source_version} →{" "}
        {response.target_version}
      </p>

      <div>
        <h2 className="text-base font-medium">{m.upgrade_actionsTitle()}</h2>
        {response.actions.length === 0 ? (
          <p className="text-sm text-[--color-fg-muted]">
            {m.upgrade_noActions()}
          </p>
        ) : (
          <ul className="m-0 list-disc pl-5 text-sm">
            {response.actions.map((action) => (
              <li key={action.path}>
                <ActionLabel action={action} />
              </li>
            ))}
          </ul>
        )}
      </div>

      <div>
        <h2 className="text-base font-medium">{m.upgrade_concernsTitle()}</h2>
        {response.concerns.length === 0 ? (
          <p className="text-sm text-[--color-fg-muted]">
            {m.upgrade_noConcerns()}
          </p>
        ) : (
          <ul className="m-0 list-disc pl-5 text-sm text-[--color-error]">
            {response.concerns.map((concern, idx) => (
              <li key={`${concern.kind}-${idx}`}>
                <ConcernLabel concern={concern} />
              </li>
            ))}
          </ul>
        )}
      </div>

      {response.preserved_user_paths.length > 0 && (
        <div>
          <h2 className="text-base font-medium">
            {m.upgrade_preservedTitle()}
          </h2>
          <ul className="m-0 list-disc pl-5 text-sm text-[--color-success]">
            {response.preserved_user_paths.map((path) => (
              <li key={path}>{path}</li>
            ))}
          </ul>
        </div>
      )}
    </section>
  );
}

function ActionLabel({ action }: { action: AssetAction }): ReactNode {
  switch (action.action) {
    case "create":
      return `${m.upgrade_actionCreate()}: ${action.path}`;
    case "update":
      return `${m.upgrade_actionUpdate()}: ${action.path}`;
    case "remove":
      return `${m.upgrade_actionRemove()}: ${action.path}`;
    case "preserve":
      return `${m.upgrade_actionPreserve()}: ${action.path}`;
  }
}

function ConcernLabel({ concern }: { concern: MigrationConcern }): ReactNode {
  return `${concern.kind}: ${concern.detail}`;
}
