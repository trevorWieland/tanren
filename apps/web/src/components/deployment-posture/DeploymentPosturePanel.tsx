"use client";

import { useState } from "react";
import type { ReactNode } from "react";

import {
  AccountRequestError,
  getDeploymentPosture,
  listDeploymentPosturesCached,
  setDeploymentPosture,
  type DeploymentPosture,
  type DeploymentPostureListResponse,
  type SetDeploymentPostureResponse,
  type SupportedDeploymentPosture,
} from "@/app/lib/account-client";
import * as m from "@/i18n/paraglide/messages";

const DEFAULT_POSTURES: DeploymentPosture[] = [
  "hosted",
  "self_hosted",
  "local_only",
];

export function DeploymentPosturePanel(): ReactNode {
  const [accountId, setAccountId] = useState("");
  const [selectedPosture, setSelectedPosture] =
    useState<DeploymentPosture>("hosted");
  const [supported, setSupported] = useState<SupportedDeploymentPosture[]>([]);
  const [current, setCurrent] = useState<SetDeploymentPostureResponse | null>(
    null,
  );
  const [postureError, setPostureError] = useState<string | null>(null);
  const [postureNotice, setPostureNotice] = useState<string | null>(null);
  const [loadingSupported, setLoadingSupported] = useState(false);
  const [loadingCurrent, setLoadingCurrent] = useState(false);
  const [savingPosture, setSavingPosture] = useState(false);

  const ensureSupported = async (): Promise<boolean> => {
    if (supported.length > 0) {
      return true;
    }
    setLoadingSupported(true);
    try {
      const response = await listDeploymentPosturesCached();
      setSupported(response.supported);
      return true;
    } catch (reason: unknown) {
      const message =
        reason instanceof AccountRequestError
          ? reason.message
          : reason instanceof Error
            ? reason.message
            : String(reason);
      setPostureError(message);
      return false;
    } finally {
      setLoadingSupported(false);
    }
  };

  const loadCurrent = async (): Promise<void> => {
    const trimmed = accountId.trim();
    if (trimmed === "") {
      setPostureError(m.posture_accountIdRequired());
      return;
    }
    setPostureError(null);
    setPostureNotice(null);
    if (!(await ensureSupported())) {
      return;
    }
    setLoadingCurrent(true);
    try {
      const response = await getDeploymentPosture("account", trimmed);
      setCurrent(response.current);
      if (response.current === null) {
        setPostureNotice(m.posture_notSet());
      }
    } catch (reason: unknown) {
      const message =
        reason instanceof AccountRequestError
          ? reason.message
          : reason instanceof Error
            ? reason.message
            : String(reason);
      setPostureError(message);
    } finally {
      setLoadingCurrent(false);
    }
  };

  const savePosture = async (): Promise<void> => {
    const trimmed = accountId.trim();
    if (trimmed === "") {
      setPostureError(m.posture_accountIdRequired());
      return;
    }
    setPostureError(null);
    setPostureNotice(null);
    if (!(await ensureSupported())) {
      return;
    }
    setSavingPosture(true);
    try {
      const response = await setDeploymentPosture({
        scope: { scope: "account", account_id: trimmed },
        posture: selectedPosture,
      });
      setCurrent(response);
      setPostureNotice(m.posture_saved());
    } catch (reason: unknown) {
      const message =
        reason instanceof AccountRequestError
          ? reason.message
          : reason instanceof Error
            ? reason.message
            : String(reason);
      setPostureError(message);
    } finally {
      setSavingPosture(false);
    }
  };

  const formatAvailableCaps = (caps: string[]): string =>
    caps.length === 0 ? m.posture_none() : caps.join(", ");

  const formatUnavailableCaps = (
    unavailable: DeploymentPostureListResponse["supported"][number]["capability_summary"]["unavailable"],
  ): string =>
    unavailable.length === 0
      ? m.posture_none()
      : unavailable
          .map(
            (capability) => `${capability.capability} (${capability.reason})`,
          )
          .join(", ");

  const postureOptions =
    supported.length === 0
      ? DEFAULT_POSTURES
      : supported.map((item) => item.posture);

  return (
    <section className="rounded-md border border-[--color-border] bg-[--color-bg-surface] p-6">
      <h2 className="mb-4 text-xl font-semibold">{m.posture_title()}</h2>
      <p className="mb-4 text-sm text-[--color-fg-muted]">
        {m.posture_subtitle()}
      </p>
      <div className="mb-4 grid gap-3 sm:grid-cols-[2fr_1fr_auto_auto]">
        <input
          value={accountId}
          onChange={(event) => setAccountId(event.target.value)}
          placeholder={m.posture_accountIdPlaceholder()}
          className="rounded border border-[--color-border] bg-transparent px-3 py-2 font-mono text-sm"
        />
        <select
          value={selectedPosture}
          onChange={(event) =>
            setSelectedPosture(event.target.value as DeploymentPosture)
          }
          className="rounded border border-[--color-border] bg-transparent px-3 py-2 text-sm"
        >
          {postureOptions.map((posture) => (
            <option key={posture} value={posture}>
              {posture}
            </option>
          ))}
        </select>
        <button
          type="button"
          onClick={() => {
            void loadCurrent();
          }}
          className="rounded border border-[--color-border] px-3 py-2 text-sm"
        >
          {loadingCurrent ? m.posture_loading() : m.posture_load()}
        </button>
        <button
          type="button"
          onClick={() => {
            void savePosture();
          }}
          className="rounded border border-[--color-border] px-3 py-2 text-sm"
        >
          {savingPosture ? m.posture_saving() : m.posture_save()}
        </button>
      </div>
      <div className="space-y-2 text-sm">
        <p className="font-semibold">{m.posture_supported()}</p>
        {supported.length === 0 ? (
          <button
            type="button"
            onClick={() => {
              void ensureSupported();
            }}
            className="rounded border border-[--color-border] px-3 py-2 text-sm"
          >
            {loadingSupported
              ? m.posture_loadingSupported()
              : m.posture_loadSupported()}
          </button>
        ) : null}
        {supported.map((item) => (
          <div
            key={item.posture}
            className="rounded border border-[--color-border] p-3"
          >
            <p className="font-mono">{item.posture}</p>
            <p>
              {m.posture_available()}:{" "}
              {formatAvailableCaps(item.capability_summary.available)}
            </p>
            <p>
              {m.posture_unavailable()}:{" "}
              {formatUnavailableCaps(item.capability_summary.unavailable)}
            </p>
          </div>
        ))}
        <p className="pt-2 font-semibold">{m.posture_current()}</p>
        {current === null ? (
          <p className="text-[--color-fg-muted]">{m.posture_notSet()}</p>
        ) : (
          <div className="rounded border border-[--color-border] p-3">
            <p className="font-mono">{current.posture}</p>
            <p>
              {m.posture_available()}:{" "}
              {formatAvailableCaps(current.capability_summary.available)}
            </p>
            <p>
              {m.posture_unavailable()}:{" "}
              {formatUnavailableCaps(current.capability_summary.unavailable)}
            </p>
          </div>
        )}
        {postureNotice !== null ? (
          <p className="text-[--color-fg-muted]">{postureNotice}</p>
        ) : null}
        {postureError !== null ? (
          <p className="text-[--color-error]">{postureError}</p>
        ) : null}
      </div>
    </section>
  );
}
