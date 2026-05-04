"use client";

import { useId, useState, useTransition } from "react";
import type { FormEvent, ReactNode } from "react";

import {
  PostureRequestError,
  describePostureFailure,
  setPosture,
  type PostureView,
  type Posture,
  type CapabilityCategory,
  type CapabilityAvailability,
  type SetPostureResponse,
} from "@/app/lib/posture-client";
import * as m from "@/i18n/paraglide/messages";

const buttonClass =
  "rounded-md border border-[--color-border] bg-[--color-accent] px-4 py-2 text-base font-medium text-[--color-accent-fg] transition-colors hover:bg-[--color-accent-hover] disabled:opacity-60";

function postureLabel(posture: Posture): string {
  const key = `posture_variant_${posture}`;
  const lookup = m as unknown as Record<string, (() => string) | undefined>;
  const fn = lookup[key];
  return typeof fn === "function" ? fn() : posture;
}

function categoryLabel(category: CapabilityCategory): string {
  const key = `posture_category_${category}`;
  const lookup = m as unknown as Record<string, (() => string) | undefined>;
  const fn = lookup[key];
  return typeof fn === "function" ? fn() : category;
}

function availabilityLabel(availability: CapabilityAvailability): string {
  return availability.status === "available"
    ? m.posture_cap_available()
    : m.posture_cap_unavailable();
}

export interface PosturePickerProps {
  postures: PostureView[];
  current: PostureView | null;
}

export function PosturePicker({
  postures,
  current,
}: PosturePickerProps): ReactNode {
  const baseId = useId();
  const errorId = `${baseId}-error`;

  const [selected, setSelected] = useState<string>(current?.posture ?? "");
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [pending, startTransition] = useTransition();
  const [changeResult, setChangeResult] = useState<SetPostureResponse | null>(
    null,
  );

  function onSubmit(event: FormEvent<HTMLFormElement>): void {
    event.preventDefault();
    setErrorMessage(null);
    if (!selected) {
      setErrorMessage(m.posture_required());
      return;
    }
    startTransition(async () => {
      try {
        const result = await setPosture(selected);
        setChangeResult(result);
      } catch (cause: unknown) {
        if (cause instanceof PostureRequestError) {
          setErrorMessage(describePostureFailure(cause.failure));
        } else if (cause instanceof Error) {
          setErrorMessage(cause.message);
        } else {
          setErrorMessage(m.posture_failed());
        }
      }
    });
  }

  const errorActive = errorMessage !== null;
  const activeCurrent = changeResult?.current ?? current;

  return (
    <form
      onSubmit={onSubmit}
      noValidate
      aria-label={m.posture_formLabel()}
      className="flex w-full max-w-2xl flex-col gap-6"
    >
      {activeCurrent && (
        <div className="rounded-md border border-[--color-border] bg-[--color-bg-surface] px-4 py-3">
          <p className="m-0 text-sm text-[--color-fg-muted]">
            {m.posture_currentLabel()}
          </p>
          <p className="m-0 text-base font-medium text-[--color-fg-default]">
            {postureLabel(activeCurrent.posture)}
          </p>
          {changeResult && (
            <p className="m-0 mt-1 text-sm text-[--color-success]">
              {m.posture_changeSuccess()}
            </p>
          )}
        </div>
      )}

      <fieldset className="m-0 flex flex-col gap-3 border-0 p-0">
        {postures.map((pv) => {
          const inputId = `${baseId}-${pv.posture}`;
          const isSelected = selected === pv.posture;
          return (
            <label
              key={pv.posture}
              htmlFor={inputId}
              className={`flex cursor-pointer flex-col gap-2 rounded-md border px-4 py-3 ${
                isSelected
                  ? "border-[--color-accent] bg-[--color-bg-elevated]"
                  : "border-[--color-border] bg-[--color-bg-surface]"
              }`}
            >
              <div className="flex items-center gap-2">
                <input
                  id={inputId}
                  type="radio"
                  name="posture"
                  value={pv.posture}
                  checked={isSelected}
                  onChange={() => {
                    setSelected(pv.posture);
                  }}
                  aria-describedby={errorActive ? errorId : undefined}
                  className="accent-[--color-accent]"
                />
                <span className="text-base font-medium text-[--color-fg-default]">
                  {postureLabel(pv.posture)}
                </span>
              </div>
              <ul className="m-0 grid list-none grid-cols-2 gap-x-4 gap-y-1 pl-6">
                {pv.capabilities.map((cap) => (
                  <li
                    key={cap.category}
                    className="flex items-center gap-1 text-sm"
                  >
                    <span
                      className={
                        cap.availability.status === "available"
                          ? "text-[--color-success]"
                          : "text-[--color-error]"
                      }
                    >
                      {availabilityLabel(cap.availability)}
                    </span>
                    <span className="text-[--color-fg-muted]">
                      {categoryLabel(cap.category)}
                    </span>
                  </li>
                ))}
              </ul>
            </label>
          );
        })}
      </fieldset>

      <button type="submit" disabled={pending} className={buttonClass}>
        {pending ? m.posture_submitting() : m.posture_submit()}
      </button>
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
    </form>
  );
}
