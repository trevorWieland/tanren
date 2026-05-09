"use client";

import { useEffect, useState } from "react";
import type { ReactNode } from "react";

import {
  addUserCredential,
  fetchHealth,
  listUserCredentials as listUserCredentialsApi,
  listUserSettings as listUserSettingsApi,
  removeUserCredential as removeUserCredentialApi,
  removeUserSetting as removeUserSettingApi,
  type HealthReport,
  type UserCredentialKind,
  type UserCredentialView,
  type UserSettingKey,
  type UserSettingView,
  upsertUserSetting,
  updateUserCredential as updateUserCredentialApi,
} from "@/app/lib/account-client";
import * as m from "@/i18n/paraglide/messages";

type CredentialPanel = "list" | "add" | "update" | "remove";

export default function ConfigurationAccountPage(): ReactNode {
  const [report, setReport] = useState<HealthReport | null>(null);
  const [healthError, setHealthError] = useState<string | null>(null);

  const [uiError, setUiError] = useState<string | null>(null);
  const [uiMessage, setUiMessage] = useState<string | null>(null);

  const [settings, setSettings] = useState<UserSettingView[]>([]);
  const [credentials, setCredentials] = useState<UserCredentialView[]>([]);

  const [settingKey, setSettingKey] = useState<UserSettingKey>("theme");
  const [settingValue, setSettingValue] = useState("system");
  const [removeSettingKey, setRemoveSettingKey] =
    useState<UserSettingKey>("theme");

  const [credentialPanel, setCredentialPanel] =
    useState<CredentialPanel>("list");
  const [credentialKind, setCredentialKind] =
    useState<UserCredentialKind>("provider_api_token");
  const [credentialValue, setCredentialValue] = useState("");
  const [credentialItemId, setCredentialItemId] = useState("");

  const [busy, setBusy] = useState(false);

  useEffect(() => {
    let cancelled = false;
    void fetchHealth()
      .then((data) => {
        if (!cancelled) {
          setReport(data);
        }
      })
      .catch((reason: unknown) => {
        if (!cancelled) {
          setHealthError(
            reason instanceof Error ? reason.message : String(reason),
          );
        }
      });
    return () => {
      cancelled = true;
    };
  }, []);

  async function listSettings(): Promise<void> {
    setBusy(true);
    setUiError(null);
    setUiMessage(null);
    try {
      const body = await listUserSettingsApi();
      setSettings(body.items);
      setUiMessage("Loaded settings.");
    } catch (error: unknown) {
      setUiError(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  }

  async function setUserSetting(): Promise<void> {
    const rawValue = settingValue.trim();
    if (rawValue === "") {
      setUiError("validation_failed: value is required");
      return;
    }
    setBusy(true);
    setUiError(null);
    setUiMessage(null);
    try {
      const valuePayload =
        settingKey === "theme"
          ? rawValue === "system" || rawValue === "light" || rawValue === "dark"
            ? ({ kind: "theme", value: rawValue } as const)
            : null
          : ({ kind: "editor", value: rawValue } as const);
      if (valuePayload === null) {
        setUiError(
          "validation_failed: theme must be one of system, light, dark",
        );
        return;
      }
      await upsertUserSetting({
        key: settingKey,
        value: valuePayload,
      });
      setUiMessage("Setting saved.");
      await listSettings();
    } catch (error: unknown) {
      setUiError(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  }

  async function removeUserSetting(): Promise<void> {
    setBusy(true);
    setUiError(null);
    setUiMessage(null);
    try {
      await removeUserSettingApi(removeSettingKey);
      setUiMessage("Setting removed.");
      await listSettings();
    } catch (error: unknown) {
      setUiError(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  }

  async function listCredentials(): Promise<void> {
    setBusy(true);
    setUiError(null);
    setUiMessage(null);
    try {
      const body = await listUserCredentialsApi();
      setCredentials(body.items);
      setUiMessage("Loaded credential metadata.");
    } catch (error: unknown) {
      setUiError(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  }

  async function addCredential(): Promise<void> {
    const secret = credentialValue;
    if (secret.trim() === "") {
      setUiError("validation_failed: credential value is required");
      return;
    }
    setBusy(true);
    setUiError(null);
    setUiMessage(null);
    try {
      const response = await addUserCredential({
        kind: credentialKind,
        value: secret,
      });
      setUiMessage(
        `Credential saved: ${response.item.id} (${response.item.kind}).`,
      );
      await listCredentials();
    } catch (error: unknown) {
      setUiError(error instanceof Error ? error.message : String(error));
    } finally {
      setCredentialValue("");
      setBusy(false);
    }
  }

  async function updateCredential(): Promise<void> {
    const itemId = credentialItemId.trim();
    if (itemId === "") {
      setUiError("validation_failed: item id is required");
      return;
    }
    const secret = credentialValue;
    if (secret.trim() === "") {
      setUiError("validation_failed: credential value is required");
      return;
    }
    setBusy(true);
    setUiError(null);
    setUiMessage(null);
    try {
      const response = await updateUserCredentialApi(itemId, {
        value: secret,
      });
      setUiMessage(
        `Credential updated: ${response.item.id} (${response.item.kind}).`,
      );
      await listCredentials();
    } catch (error: unknown) {
      setUiError(error instanceof Error ? error.message : String(error));
    } finally {
      setCredentialValue("");
      setBusy(false);
    }
  }

  async function removeCredential(): Promise<void> {
    const itemId = credentialItemId.trim();
    if (itemId === "") {
      setUiError("validation_failed: item id is required");
      return;
    }
    setBusy(true);
    setUiError(null);
    setUiMessage(null);
    try {
      await removeUserCredentialApi(itemId);
      setUiMessage(`Credential removed: ${itemId}.`);
      await listCredentials();
    } catch (error: unknown) {
      setUiError(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  }

  return (
    <main className="mx-auto flex min-h-screen w-full max-w-6xl flex-col gap-6 px-4 py-6">
      <header className="rounded-md border border-[--color-border] bg-[--color-bg-surface] px-4 py-3">
        <h1 className="text-2xl font-semibold">{m.app_title()}</h1>
        <p className="text-sm text-[--color-fg-muted]">
          User-tier configuration workspace for the signed-in account
        </p>
      </header>

      <section className="grid gap-4 md:grid-cols-1">
        <article className="rounded-md border border-[--color-border] bg-[--color-bg-surface] p-4">
          <h2 className="mb-2 text-sm font-semibold">Health</h2>
          {report !== null ? (
            <pre className="m-0 overflow-x-auto text-xs">
              {JSON.stringify(report, null, 2)}
            </pre>
          ) : healthError !== null ? (
            <p className="text-sm text-[--color-error]">
              {m.app_health_unreachable()}: {healthError}
            </p>
          ) : (
            <p className="text-sm text-[--color-fg-muted]">
              {m.app_health_loading()}
            </p>
          )}
        </article>
      </section>

      <section className="rounded-md border border-[--color-border] bg-[--color-bg-surface] p-4">
        <h2 className="mb-3 text-sm font-semibold">User settings</h2>
        <div className="mb-3 flex flex-wrap items-center gap-2">
          <button
            type="button"
            onClick={() => void listSettings()}
            disabled={busy}
            className="rounded-md bg-[--color-accent] px-3 py-1.5 text-sm text-[--color-accent-fg] disabled:opacity-60"
          >
            List
          </button>
          <select
            value={settingKey}
            onChange={(event) =>
              setSettingKey(event.target.value as UserSettingKey)
            }
            className="rounded-md border border-[--color-border] bg-[--color-bg-canvas] px-2 py-1.5 text-sm"
          >
            <option value="theme">theme</option>
            <option value="editor">editor</option>
          </select>
          <input
            value={settingValue}
            onChange={(event) => setSettingValue(event.target.value)}
            className="min-w-48 flex-1 rounded-md border border-[--color-border] bg-[--color-bg-canvas] px-3 py-1.5 text-sm"
            placeholder={
              settingKey === "theme"
                ? "system | light | dark"
                : "editor command"
            }
          />
          <button
            type="button"
            onClick={() => void setUserSetting()}
            disabled={busy}
            className="rounded-md bg-[--color-accent] px-3 py-1.5 text-sm text-[--color-accent-fg] disabled:opacity-60"
          >
            Set
          </button>
        </div>
        <div className="mb-3 flex flex-wrap items-center gap-2">
          <select
            value={removeSettingKey}
            onChange={(event) =>
              setRemoveSettingKey(event.target.value as UserSettingKey)
            }
            className="rounded-md border border-[--color-border] bg-[--color-bg-canvas] px-2 py-1.5 text-sm"
          >
            <option value="theme">theme</option>
            <option value="editor">editor</option>
          </select>
          <button
            type="button"
            onClick={() => void removeUserSetting()}
            disabled={busy}
            className="rounded-md border border-[--color-border] px-3 py-1.5 text-sm disabled:opacity-60"
          >
            Remove
          </button>
        </div>
        <pre className="m-0 max-h-40 overflow-auto rounded-md border border-[--color-border] bg-[--color-bg-canvas] p-3 text-xs">
          {JSON.stringify(settings, null, 2)}
        </pre>
      </section>

      <section className="rounded-md border border-[--color-border] bg-[--color-bg-surface] p-4">
        <h2 className="mb-3 text-sm font-semibold">Credentials</h2>
        <div className="mb-3 flex flex-wrap gap-2">
          {(["list", "add", "update", "remove"] as const).map((panel) => (
            <button
              key={panel}
              type="button"
              onClick={() => setCredentialPanel(panel)}
              className={`rounded-md px-3 py-1.5 text-sm ${credentialPanel === panel ? "bg-[--color-accent] text-[--color-accent-fg]" : "border border-[--color-border]"}`}
            >
              {panel}
            </button>
          ))}
        </div>

        {credentialPanel === "list" ? (
          <button
            type="button"
            onClick={() => void listCredentials()}
            disabled={busy}
            className="mb-3 rounded-md bg-[--color-accent] px-3 py-1.5 text-sm text-[--color-accent-fg] disabled:opacity-60"
          >
            List metadata
          </button>
        ) : null}

        {credentialPanel === "add" ? (
          <div className="mb-3 flex flex-wrap items-center gap-2">
            <select
              value={credentialKind}
              onChange={(event) =>
                setCredentialKind(event.target.value as UserCredentialKind)
              }
              className="rounded-md border border-[--color-border] bg-[--color-bg-canvas] px-2 py-1.5 text-sm"
            >
              <option value="provider_api_token">provider_api_token</option>
              <option value="harness_api_token">harness_api_token</option>
            </select>
            <input
              type="password"
              value={credentialValue}
              onChange={(event) => setCredentialValue(event.target.value)}
              className="min-w-48 flex-1 rounded-md border border-[--color-border] bg-[--color-bg-canvas] px-3 py-1.5 text-sm"
              placeholder="Credential secret"
            />
            <button
              type="button"
              onClick={() => void addCredential()}
              disabled={busy}
              className="rounded-md bg-[--color-accent] px-3 py-1.5 text-sm text-[--color-accent-fg] disabled:opacity-60"
            >
              Add
            </button>
          </div>
        ) : null}

        {credentialPanel === "update" ? (
          <div className="mb-3 flex flex-wrap items-center gap-2">
            <input
              value={credentialItemId}
              onChange={(event) => setCredentialItemId(event.target.value)}
              className="min-w-48 flex-1 rounded-md border border-[--color-border] bg-[--color-bg-canvas] px-3 py-1.5 text-sm"
              placeholder="Credential item id"
            />
            <input
              type="password"
              value={credentialValue}
              onChange={(event) => setCredentialValue(event.target.value)}
              className="min-w-48 flex-1 rounded-md border border-[--color-border] bg-[--color-bg-canvas] px-3 py-1.5 text-sm"
              placeholder="New credential secret"
            />
            <button
              type="button"
              onClick={() => void updateCredential()}
              disabled={busy}
              className="rounded-md bg-[--color-accent] px-3 py-1.5 text-sm text-[--color-accent-fg] disabled:opacity-60"
            >
              Update
            </button>
          </div>
        ) : null}

        {credentialPanel === "remove" ? (
          <div className="mb-3 flex flex-wrap items-center gap-2">
            <input
              value={credentialItemId}
              onChange={(event) => setCredentialItemId(event.target.value)}
              className="min-w-48 flex-1 rounded-md border border-[--color-border] bg-[--color-bg-canvas] px-3 py-1.5 text-sm"
              placeholder="Credential item id"
            />
            <button
              type="button"
              onClick={() => void removeCredential()}
              disabled={busy}
              className="rounded-md border border-[--color-border] px-3 py-1.5 text-sm disabled:opacity-60"
            >
              Remove
            </button>
          </div>
        ) : null}

        <pre className="m-0 max-h-52 overflow-auto rounded-md border border-[--color-border] bg-[--color-bg-canvas] p-3 text-xs">
          {JSON.stringify(credentials, null, 2)}
        </pre>
      </section>

      {uiError !== null ? (
        <p className="text-sm text-[--color-error]">{uiError}</p>
      ) : null}
      {uiMessage !== null ? (
        <p className="text-sm text-[--color-success]">{uiMessage}</p>
      ) : null}
    </main>
  );
}
