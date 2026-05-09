"use client";

import { useEffect, useState } from "react";
import type { ReactNode } from "react";

import {
  addUserCredential,
  describeFailure,
  discoverConfigurationAccess,
  fetchHealth,
  listUserCredentials as listUserCredentialsApi,
  listUserSettings as listUserSettingsApi,
  removeUserCredential as removeUserCredentialApi,
  removeUserSetting as removeUserSettingApi,
  type HealthReport,
  upsertUserSetting,
  updateUserCredential as updateUserCredentialApi,
} from "@/app/lib/account-client";
import type {
  ConfigurationCapabilities,
  UserCredentialKind,
  UserCredentialView,
  UserSettingKey,
  UserSettingView,
} from "@/app/lib/api-contracts";
import * as m from "@/i18n/paraglide/messages";

type CredentialPanel = "list" | "add" | "update" | "remove";

export default function ConfigurationAccountPage(): ReactNode {
  const [report, setReport] = useState<HealthReport | null>(null);
  const [healthError, setHealthError] = useState<string | null>(null);

  const [uiError, setUiError] = useState<string | null>(null);
  const [uiMessage, setUiMessage] = useState<string | null>(null);

  const [capabilities, setCapabilities] =
    useState<ConfigurationCapabilities | null>(null);
  const [settingsAccessError, setSettingsAccessError] = useState<string | null>(
    null,
  );
  const [credentialsAccessError, setCredentialsAccessError] = useState<
    string | null
  >(null);
  const [capabilitiesAccessError, setCapabilitiesAccessError] = useState<
    string | null
  >(null);

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

  const settingActions = capabilities?.settings.allowed_actions ?? [];
  const itemActions = capabilities?.user_items.allowed_actions ?? [];
  const canReadSettings = settingActions.includes("read");
  const canWriteSettings = settingActions.includes("create_or_update");
  const canDeleteSettings = settingActions.includes("delete");
  const canReadCredentials = itemActions.includes("read");
  const canCreateCredentials = itemActions.includes("create");
  const canUpdateCredentials = itemActions.includes("update");
  const canDeleteCredentials = itemActions.includes("delete");

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

    void discoverConfigurationAccess()
      .then((result) => {
        if (cancelled) {
          return;
        }
        setCapabilities(result.capabilities);
        setCapabilitiesAccessError(
          result.capabilities_failure === null
            ? null
            : describeFailure(result.capabilities_failure),
        );
        setSettings(result.settings);
        setCredentials(result.credentials);
        setSettingsAccessError(
          result.settings_failure === null
            ? null
            : describeFailure(result.settings_failure),
        );
        setCredentialsAccessError(
          result.credentials_failure === null
            ? null
            : describeFailure(result.credentials_failure),
        );
      })
      .catch((reason: unknown) => {
        if (cancelled) {
          return;
        }
        const details =
          reason instanceof Error ? reason.message : String(reason);
        setCapabilitiesAccessError(details);
        setSettingsAccessError(`${m.config_access_check_failed()}: ${details}`);
        setCredentialsAccessError(
          `${m.config_access_check_failed()}: ${details}`,
        );
      });

    return () => {
      cancelled = true;
    };
  }, []);

  async function listSettings(): Promise<void> {
    if (!canReadSettings) {
      setUiError(m.config_settings_capability_required_read());
      return;
    }
    setBusy(true);
    setUiError(null);
    setUiMessage(null);
    try {
      const body = await listUserSettingsApi();
      setSettings(body.items);
      setUiMessage(m.config_settings_list_success());
    } catch (error: unknown) {
      setUiError(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  }

  async function setUserSetting(): Promise<void> {
    if (!canWriteSettings) {
      setUiError(m.config_settings_capability_required_write());
      return;
    }
    const rawValue = settingValue.trim();
    if (rawValue === "") {
      setUiError(m.config_settings_validation_value_required());
      return;
    }

    const valuePayload =
      settingKey === "theme"
        ? rawValue === "system" || rawValue === "light" || rawValue === "dark"
          ? ({ kind: "theme", value: rawValue } as const)
          : null
        : ({ kind: "editor", value: rawValue } as const);
    if (valuePayload === null) {
      setUiError(m.config_settings_validation_theme_invalid());
      return;
    }

    setBusy(true);
    setUiError(null);
    setUiMessage(null);
    try {
      await upsertUserSetting({
        key: settingKey,
        value: valuePayload,
      });
      setUiMessage(m.config_settings_set_success());
      await listSettings();
    } catch (error: unknown) {
      setUiError(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  }

  async function removeUserSetting(): Promise<void> {
    if (!canDeleteSettings) {
      setUiError(m.config_settings_capability_required_delete());
      return;
    }
    setBusy(true);
    setUiError(null);
    setUiMessage(null);
    try {
      await removeUserSettingApi(removeSettingKey);
      setUiMessage(m.config_settings_remove_success());
      await listSettings();
    } catch (error: unknown) {
      setUiError(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  }

  async function listCredentials(): Promise<void> {
    if (!canReadCredentials) {
      setUiError(m.config_credentials_capability_required_read());
      return;
    }
    setBusy(true);
    setUiError(null);
    setUiMessage(null);
    try {
      const body = await listUserCredentialsApi();
      setCredentials(body.items);
      setUiMessage(m.config_credentials_list_success());
    } catch (error: unknown) {
      setUiError(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  }

  async function addCredential(): Promise<void> {
    if (!canCreateCredentials) {
      setUiError(m.config_credentials_capability_required_create());
      return;
    }
    const secret = credentialValue;
    if (secret.trim() === "") {
      setUiError(m.config_credentials_validation_value_required());
      return;
    }
    setBusy(true);
    setUiError(null);
    setUiMessage(null);
    try {
      await addUserCredential({
        kind: credentialKind,
        value: secret,
      });
      setUiMessage(m.config_credentials_add_success());
      await listCredentials();
    } catch (error: unknown) {
      setUiError(error instanceof Error ? error.message : String(error));
    } finally {
      setCredentialValue("");
      setBusy(false);
    }
  }

  async function updateCredential(): Promise<void> {
    if (!canUpdateCredentials) {
      setUiError(m.config_credentials_capability_required_update());
      return;
    }
    const itemId = credentialItemId.trim();
    if (itemId === "") {
      setUiError(m.config_credentials_validation_item_id_required());
      return;
    }
    const secret = credentialValue;
    if (secret.trim() === "") {
      setUiError(m.config_credentials_validation_value_required());
      return;
    }
    setBusy(true);
    setUiError(null);
    setUiMessage(null);
    try {
      await updateUserCredentialApi(itemId, {
        value: secret,
      });
      setUiMessage(m.config_credentials_update_success());
      await listCredentials();
    } catch (error: unknown) {
      setUiError(error instanceof Error ? error.message : String(error));
    } finally {
      setCredentialValue("");
      setBusy(false);
    }
  }

  async function removeCredential(): Promise<void> {
    if (!canDeleteCredentials) {
      setUiError(m.config_credentials_capability_required_delete());
      return;
    }
    const itemId = credentialItemId.trim();
    if (itemId === "") {
      setUiError(m.config_credentials_validation_item_id_required());
      return;
    }
    setBusy(true);
    setUiError(null);
    setUiMessage(null);
    try {
      await removeUserCredentialApi(itemId);
      setUiMessage(m.config_credentials_remove_success());
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
          {m.config_page_subtitle()}
        </p>
      </header>

      <section className="grid gap-4 md:grid-cols-1">
        <article className="rounded-md border border-[--color-border] bg-[--color-bg-surface] p-4">
          <h2 className="mb-2 text-sm font-semibold">
            {m.config_health_title()}
          </h2>
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
        <h2 className="mb-3 text-sm font-semibold">
          {m.config_settings_title()}
        </h2>

        {capabilities === null ? (
          <p className="mb-3 text-sm text-[--color-fg-muted]">
            {m.config_access_loading()}
          </p>
        ) : null}

        {capabilitiesAccessError !== null ? (
          <p className="mb-3 text-sm text-[--color-fg-muted]">
            {m.config_access_check_failed()}: {capabilitiesAccessError}
          </p>
        ) : null}

        {settingsAccessError !== null ? (
          <p className="mb-3 text-sm text-[--color-fg-muted]">
            {m.config_settings_access_limited()}: {settingsAccessError}
          </p>
        ) : null}

        <div className="mb-3 flex flex-wrap items-center gap-2">
          <button
            type="button"
            onClick={() => void listSettings()}
            disabled={busy || !canReadSettings}
            className="rounded-md bg-[--color-accent] px-3 py-1.5 text-sm text-[--color-accent-fg] disabled:opacity-60"
          >
            {m.config_settings_list_button()}
          </button>
          <select
            value={settingKey}
            onChange={(event) =>
              setSettingKey(event.target.value as UserSettingKey)
            }
            className="rounded-md border border-[--color-border] bg-[--color-bg-canvas] px-2 py-1.5 text-sm"
          >
            <option value="theme">{m.config_setting_key_theme()}</option>
            <option value="editor">{m.config_setting_key_editor()}</option>
          </select>
          <input
            value={settingValue}
            onChange={(event) => setSettingValue(event.target.value)}
            className="min-w-48 flex-1 rounded-md border border-[--color-border] bg-[--color-bg-canvas] px-3 py-1.5 text-sm"
            placeholder={
              settingKey === "theme"
                ? m.config_settings_placeholder_theme_values()
                : m.config_settings_placeholder_editor_command()
            }
          />
          <button
            type="button"
            onClick={() => void setUserSetting()}
            disabled={busy || !canWriteSettings}
            className="rounded-md bg-[--color-accent] px-3 py-1.5 text-sm text-[--color-accent-fg] disabled:opacity-60"
          >
            {m.config_settings_set_button()}
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
            <option value="theme">{m.config_setting_key_theme()}</option>
            <option value="editor">{m.config_setting_key_editor()}</option>
          </select>
          <button
            type="button"
            onClick={() => void removeUserSetting()}
            disabled={busy || !canDeleteSettings}
            className="rounded-md border border-[--color-border] px-3 py-1.5 text-sm disabled:opacity-60"
          >
            {m.config_settings_remove_button()}
          </button>
        </div>
        <pre className="m-0 max-h-40 overflow-auto rounded-md border border-[--color-border] bg-[--color-bg-canvas] p-3 text-xs">
          {JSON.stringify(settings, null, 2)}
        </pre>
      </section>

      <section className="rounded-md border border-[--color-border] bg-[--color-bg-surface] p-4">
        <h2 className="mb-3 text-sm font-semibold">
          {m.config_credentials_title()}
        </h2>

        {capabilities === null ? (
          <p className="mb-3 text-sm text-[--color-fg-muted]">
            {m.config_access_loading()}
          </p>
        ) : null}

        {capabilitiesAccessError !== null ? (
          <p className="mb-3 text-sm text-[--color-fg-muted]">
            {m.config_access_check_failed()}: {capabilitiesAccessError}
          </p>
        ) : null}

        {credentialsAccessError !== null ? (
          <p className="mb-3 text-sm text-[--color-fg-muted]">
            {m.config_credentials_access_limited()}: {credentialsAccessError}
          </p>
        ) : null}

        <div className="mb-3 flex flex-wrap gap-2">
          {["list", "add", "update", "remove"].map((panel) => {
            const panelName = panel as CredentialPanel;
            const enabled =
              panelName === "list"
                ? canReadCredentials
                : panelName === "add"
                  ? canCreateCredentials
                  : panelName === "update"
                    ? canUpdateCredentials
                    : canDeleteCredentials;
            if (!enabled) {
              return null;
            }
            const label =
              panelName === "list"
                ? m.config_credentials_panel_list()
                : panelName === "add"
                  ? m.config_credentials_panel_add()
                  : panelName === "update"
                    ? m.config_credentials_panel_update()
                    : m.config_credentials_panel_remove();
            return (
              <button
                key={panelName}
                type="button"
                onClick={() => setCredentialPanel(panelName)}
                className={`rounded-md px-3 py-1.5 text-sm ${credentialPanel === panelName ? "bg-[--color-accent] text-[--color-accent-fg]" : "border border-[--color-border]"}`}
              >
                {label}
              </button>
            );
          })}
        </div>

        {credentialPanel === "list" && canReadCredentials ? (
          <button
            type="button"
            onClick={() => void listCredentials()}
            disabled={busy}
            className="mb-3 rounded-md bg-[--color-accent] px-3 py-1.5 text-sm text-[--color-accent-fg] disabled:opacity-60"
          >
            {m.config_credentials_list_button()}
          </button>
        ) : null}

        {credentialPanel === "add" && canCreateCredentials ? (
          <div className="mb-3 flex flex-wrap items-center gap-2">
            <select
              value={credentialKind}
              onChange={(event) =>
                setCredentialKind(event.target.value as UserCredentialKind)
              }
              className="rounded-md border border-[--color-border] bg-[--color-bg-canvas] px-2 py-1.5 text-sm"
            >
              <option value="provider_api_token">
                {m.config_credential_kind_provider_api_token()}
              </option>
              <option value="harness_api_token">
                {m.config_credential_kind_harness_api_token()}
              </option>
            </select>
            <input
              type="password"
              value={credentialValue}
              onChange={(event) => setCredentialValue(event.target.value)}
              className="min-w-48 flex-1 rounded-md border border-[--color-border] bg-[--color-bg-canvas] px-3 py-1.5 text-sm"
              placeholder={m.config_credentials_placeholder_secret()}
            />
            <button
              type="button"
              onClick={() => void addCredential()}
              disabled={busy}
              className="rounded-md bg-[--color-accent] px-3 py-1.5 text-sm text-[--color-accent-fg] disabled:opacity-60"
            >
              {m.config_credentials_add_button()}
            </button>
          </div>
        ) : null}

        {credentialPanel === "update" && canUpdateCredentials ? (
          <div className="mb-3 flex flex-wrap items-center gap-2">
            <input
              value={credentialItemId}
              onChange={(event) => setCredentialItemId(event.target.value)}
              className="min-w-48 flex-1 rounded-md border border-[--color-border] bg-[--color-bg-canvas] px-3 py-1.5 text-sm"
              placeholder={m.config_credentials_placeholder_item_id()}
            />
            <input
              type="password"
              value={credentialValue}
              onChange={(event) => setCredentialValue(event.target.value)}
              className="min-w-48 flex-1 rounded-md border border-[--color-border] bg-[--color-bg-canvas] px-3 py-1.5 text-sm"
              placeholder={m.config_credentials_placeholder_new_secret()}
            />
            <button
              type="button"
              onClick={() => void updateCredential()}
              disabled={busy}
              className="rounded-md bg-[--color-accent] px-3 py-1.5 text-sm text-[--color-accent-fg] disabled:opacity-60"
            >
              {m.config_credentials_update_button()}
            </button>
          </div>
        ) : null}

        {credentialPanel === "remove" && canDeleteCredentials ? (
          <div className="mb-3 flex flex-wrap items-center gap-2">
            <input
              value={credentialItemId}
              onChange={(event) => setCredentialItemId(event.target.value)}
              className="min-w-48 flex-1 rounded-md border border-[--color-border] bg-[--color-bg-canvas] px-3 py-1.5 text-sm"
              placeholder={m.config_credentials_placeholder_item_id()}
            />
            <button
              type="button"
              onClick={() => void removeCredential()}
              disabled={busy}
              className="rounded-md border border-[--color-border] px-3 py-1.5 text-sm disabled:opacity-60"
            >
              {m.config_credentials_remove_button()}
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
