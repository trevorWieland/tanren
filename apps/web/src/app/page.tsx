"use client";

import { useEffect, useMemo, useState } from "react";
import type { ReactNode } from "react";

import { withJsonContentType } from "@/app/lib/http";
import * as m from "@/i18n/paraglide/messages";

interface HealthReport {
  status: string;
  version: string;
  contract_version: number;
}

interface FailureBody {
  code?: string;
  summary?: string;
}

interface UserSettingView {
  key: "theme" | "editor";
  value:
    | { kind: "theme"; value: "system" | "light" | "dark" }
    | { kind: "editor"; value: string };
  updated_at: string;
}

interface UserCredentialView {
  id: string;
  kind: "provider_api_token" | "harness_api_token";
  owner_scope: { scope: "user"; account_id: string };
  status: "pending" | "active" | "invalid";
  created_at: string;
  updated_at: string;
}

const API_URL = process.env["NEXT_PUBLIC_API_URL"] ?? "http://localhost:8080";

type CredentialPanel = "list" | "add" | "update" | "remove";

export default function Home(): ReactNode {
  const [report, setReport] = useState<HealthReport | null>(null);
  const [healthError, setHealthError] = useState<string | null>(null);

  const [accountId, setAccountId] = useState("");
  const [uiError, setUiError] = useState<string | null>(null);
  const [uiMessage, setUiMessage] = useState<string | null>(null);

  const [settings, setSettings] = useState<UserSettingView[]>([]);
  const [credentials, setCredentials] = useState<UserCredentialView[]>([]);

  const [settingKey, setSettingKey] = useState<"theme" | "editor">("theme");
  const [settingValue, setSettingValue] = useState("system");
  const [removeSettingKey, setRemoveSettingKey] = useState<"theme" | "editor">(
    "theme",
  );

  const [credentialPanel, setCredentialPanel] =
    useState<CredentialPanel>("list");
  const [credentialKind, setCredentialKind] = useState<
    "provider_api_token" | "harness_api_token"
  >("provider_api_token");
  const [credentialValue, setCredentialValue] = useState("");
  const [credentialItemId, setCredentialItemId] = useState("");

  const [busy, setBusy] = useState(false);

  useEffect(() => {
    let cancelled = false;
    fetch(`${API_URL}/health`, { credentials: "include" })
      .then(async (response) => {
        if (!response.ok) throw new Error(`HTTP ${response.status}`);
        return (await response.json()) as HealthReport;
      })
      .then((data) => {
        if (!cancelled) setReport(data);
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

  const trimmedAccountId = useMemo(() => accountId.trim(), [accountId]);

  async function parseFailure(response: Response): Promise<string> {
    let body: FailureBody = {};
    try {
      body = (await response.json()) as FailureBody;
    } catch {
      body = {};
    }
    const code = body.code ?? "internal_error";
    const summary = body.summary ?? `HTTP ${response.status}`;
    return `${code}: ${summary}`;
  }

  async function requestJson<T>(path: string, init: RequestInit): Promise<T> {
    const response = await fetch(`${API_URL}${path}`, {
      ...init,
      credentials: "include",
      headers: withJsonContentType(init.headers),
    });
    if (!response.ok) {
      throw new Error(await parseFailure(response));
    }
    return (await response.json()) as T;
  }

  async function listSettings(): Promise<void> {
    if (trimmedAccountId === "") {
      setUiError("validation_failed: account id is required");
      return;
    }
    setBusy(true);
    setUiError(null);
    setUiMessage(null);
    try {
      const response = await fetch(
        `${API_URL}/accounts/${encodeURIComponent(trimmedAccountId)}/user-settings`,
        {
          method: "GET",
          credentials: "include",
        },
      );
      if (!response.ok) throw new Error(await parseFailure(response));
      const body = (await response.json()) as { items: UserSettingView[] };
      setSettings(body.items);
      setUiMessage("Loaded settings.");
    } catch (error: unknown) {
      setUiError(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  }

  async function setUserSetting(): Promise<void> {
    if (trimmedAccountId === "") {
      setUiError("validation_failed: account id is required");
      return;
    }
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
          ? ({ kind: "theme", value: rawValue } as const)
          : ({ kind: "editor", value: rawValue } as const);
      await requestJson<{ setting: UserSettingView }>(
        `/accounts/${encodeURIComponent(trimmedAccountId)}/user-settings`,
        {
          method: "POST",
          body: JSON.stringify({ key: settingKey, value: valuePayload }),
        },
      );
      setUiMessage("Setting saved.");
      await listSettings();
    } catch (error: unknown) {
      setUiError(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  }

  async function removeUserSetting(): Promise<void> {
    if (trimmedAccountId === "") {
      setUiError("validation_failed: account id is required");
      return;
    }
    setBusy(true);
    setUiError(null);
    setUiMessage(null);
    try {
      const response = await fetch(
        `${API_URL}/accounts/${encodeURIComponent(trimmedAccountId)}/user-settings/${encodeURIComponent(removeSettingKey)}`,
        {
          method: "DELETE",
          credentials: "include",
        },
      );
      if (!response.ok) throw new Error(await parseFailure(response));
      setUiMessage("Setting removed.");
      await listSettings();
    } catch (error: unknown) {
      setUiError(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  }

  async function listCredentials(): Promise<void> {
    if (trimmedAccountId === "") {
      setUiError("validation_failed: account id is required");
      return;
    }
    setBusy(true);
    setUiError(null);
    setUiMessage(null);
    try {
      const response = await fetch(
        `${API_URL}/accounts/${encodeURIComponent(trimmedAccountId)}/user-credentials`,
        {
          method: "GET",
          credentials: "include",
        },
      );
      if (!response.ok) throw new Error(await parseFailure(response));
      const body = (await response.json()) as { items: UserCredentialView[] };
      setCredentials(body.items);
      setUiMessage("Loaded credential metadata.");
    } catch (error: unknown) {
      setUiError(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  }

  async function addCredential(): Promise<void> {
    if (trimmedAccountId === "") {
      setUiError("validation_failed: account id is required");
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
      const body = {
        kind: credentialKind,
        owner_scope: { scope: "user", account_id: trimmedAccountId },
        value: secret,
      };
      const response = await requestJson<{ item: UserCredentialView }>(
        `/accounts/${encodeURIComponent(trimmedAccountId)}/user-credentials`,
        {
          method: "POST",
          body: JSON.stringify(body),
        },
      );
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
    if (trimmedAccountId === "") {
      setUiError("validation_failed: account id is required");
      return;
    }
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
      const response = await requestJson<{ item: UserCredentialView }>(
        `/accounts/${encodeURIComponent(trimmedAccountId)}/user-credentials/${encodeURIComponent(itemId)}`,
        {
          method: "PUT",
          body: JSON.stringify({ value: secret }),
        },
      );
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
    if (trimmedAccountId === "") {
      setUiError("validation_failed: account id is required");
      return;
    }
    const itemId = credentialItemId.trim();
    if (itemId === "") {
      setUiError("validation_failed: item id is required");
      return;
    }
    setBusy(true);
    setUiError(null);
    setUiMessage(null);
    try {
      const response = await fetch(
        `${API_URL}/accounts/${encodeURIComponent(trimmedAccountId)}/user-credentials/${encodeURIComponent(itemId)}`,
        {
          method: "DELETE",
          credentials: "include",
        },
      );
      if (!response.ok) throw new Error(await parseFailure(response));
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
          User-tier configuration workspace
        </p>
      </header>

      <section className="grid gap-4 md:grid-cols-2">
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

        <article className="rounded-md border border-[--color-border] bg-[--color-bg-surface] p-4">
          <h2 className="mb-2 text-sm font-semibold">Scope</h2>
          <label
            className="mb-2 block text-xs text-[--color-fg-muted]"
            htmlFor="account-id"
          >
            Account id (UUID)
          </label>
          <input
            id="account-id"
            value={accountId}
            onChange={(event) => setAccountId(event.target.value)}
            className="w-full rounded-md border border-[--color-border] bg-[--color-bg-canvas] px-3 py-2 text-sm"
            placeholder="00000000-0000-0000-0000-000000000000"
          />
          <p className="mt-2 text-xs text-[--color-fg-muted]">
            Sign in first at `/sign-in` so session cookies are present.
          </p>
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
              setSettingKey(event.target.value as "theme" | "editor")
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
              setRemoveSettingKey(event.target.value as "theme" | "editor")
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
                setCredentialKind(
                  event.target.value as
                    | "provider_api_token"
                    | "harness_api_token",
                )
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
