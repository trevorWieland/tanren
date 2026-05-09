"use client";

import { useEffect, useState } from "react";
import type { ReactNode } from "react";

import { CredentialsPanel } from "@/app/configuration/account/CredentialsPanel";
import { SettingsPanel } from "@/app/configuration/account/SettingsPanel";
import {
  addUserCredential,
  AccountRequestError,
  discoverConfigurationAccess,
  listUserCredentials as listUserCredentialsApi,
  listUserSettings as listUserSettingsApi,
  removeUserCredential as removeUserCredentialApi,
  removeUserSetting as removeUserSettingApi,
  upsertUserSetting,
  updateUserCredential as updateUserCredentialApi,
} from "@/app/lib/account-client";
import type {
  AccountFailure,
  ConfigurationCapabilities,
  CreateUserCredentialInput,
  ListUserCredentialsResult,
  ListUserSettingsResult,
  UpdateUserCredentialInput,
  UpsertUserSettingInput,
} from "@/app/lib/api-contracts";
import * as m from "@/i18n/paraglide/messages";

function formatFailure(failure: AccountFailure): string {
  return `code=${failure.code}; summary=${failure.summary}`;
}

function formatRequestError(error: unknown): string {
  if (error instanceof AccountRequestError) {
    const failure = error.failure;
    return formatFailure(failure);
  }
  if (error instanceof Error) {
    return error.message;
  }
  return String(error);
}

export default function ConfigurationAccountPage(): ReactNode {
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

  const [settingsReadModel, setSettingsReadModel] =
    useState<ListUserSettingsResult | null>(null);
  const [credentialsReadModel, setCredentialsReadModel] =
    useState<ListUserCredentialsResult | null>(null);

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

    void discoverConfigurationAccess()
      .then((result) => {
        if (cancelled) {
          return;
        }
        setCapabilities(result.capabilities);
        setCapabilitiesAccessError(
          result.capabilities_failure === null
            ? null
            : `${m.config_access_check_failed()}: ${formatFailure(result.capabilities_failure)}`,
        );
        setSettingsReadModel(result.settings_read_model);
        setCredentialsReadModel(result.credentials_read_model);
        setSettingsAccessError(
          result.settings_failure === null
            ? null
            : `${m.config_settings_access_limited()}: ${formatFailure(result.settings_failure)}`,
        );
        setCredentialsAccessError(
          result.credentials_failure === null
            ? null
            : `${m.config_credentials_access_limited()}: ${formatFailure(result.credentials_failure)}`,
        );
      })
      .catch((reason: unknown) => {
        if (cancelled) {
          return;
        }
        const details = formatRequestError(reason);
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

  async function refreshSettingsReadModel(): Promise<void> {
    const body = await listUserSettingsApi();
    setSettingsReadModel(body);
    setSettingsAccessError(null);
  }

  async function refreshCredentialsReadModel(): Promise<void> {
    const body = await listUserCredentialsApi();
    setCredentialsReadModel(body);
    setCredentialsAccessError(null);
  }

  async function listSettings(): Promise<void> {
    setBusy(true);
    setUiError(null);
    setUiMessage(null);
    try {
      await refreshSettingsReadModel();
      setUiMessage(m.config_settings_list_success());
    } catch (error: unknown) {
      setUiError(formatRequestError(error));
    } finally {
      setBusy(false);
    }
  }

  async function setUserSetting(input: UpsertUserSettingInput): Promise<void> {
    setBusy(true);
    setUiError(null);
    setUiMessage(null);
    try {
      await upsertUserSetting(input);
      await refreshSettingsReadModel();
      setUiMessage(m.config_settings_set_success());
    } catch (error: unknown) {
      setUiError(formatRequestError(error));
    } finally {
      setBusy(false);
    }
  }

  async function removeUserSetting(
    key: UpsertUserSettingInput["key"],
  ): Promise<void> {
    setBusy(true);
    setUiError(null);
    setUiMessage(null);
    try {
      await removeUserSettingApi(key);
      await refreshSettingsReadModel();
      setUiMessage(m.config_settings_remove_success());
    } catch (error: unknown) {
      setUiError(formatRequestError(error));
    } finally {
      setBusy(false);
    }
  }

  async function listCredentials(): Promise<void> {
    setBusy(true);
    setUiError(null);
    setUiMessage(null);
    try {
      await refreshCredentialsReadModel();
      setUiMessage(m.config_credentials_list_success());
    } catch (error: unknown) {
      setUiError(formatRequestError(error));
    } finally {
      setBusy(false);
    }
  }

  async function addCredential(
    input: CreateUserCredentialInput,
  ): Promise<void> {
    setBusy(true);
    setUiError(null);
    setUiMessage(null);
    try {
      await addUserCredential(input);
      await refreshCredentialsReadModel();
      setUiMessage(m.config_credentials_add_success());
    } catch (error: unknown) {
      setUiError(formatRequestError(error));
    } finally {
      setBusy(false);
    }
  }

  async function updateCredential(
    itemId: string,
    input: UpdateUserCredentialInput,
  ): Promise<void> {
    setBusy(true);
    setUiError(null);
    setUiMessage(null);
    try {
      await updateUserCredentialApi(itemId, input);
      await refreshCredentialsReadModel();
      setUiMessage(m.config_credentials_update_success());
    } catch (error: unknown) {
      setUiError(formatRequestError(error));
    } finally {
      setBusy(false);
    }
  }

  async function removeCredential(itemId: string): Promise<void> {
    setBusy(true);
    setUiError(null);
    setUiMessage(null);
    try {
      await removeUserCredentialApi(itemId);
      await refreshCredentialsReadModel();
      setUiMessage(m.config_credentials_remove_success());
    } catch (error: unknown) {
      setUiError(formatRequestError(error));
    } finally {
      setBusy(false);
    }
  }

  const capabilitiesLoading = capabilities === null;
  const sharedAccessError = capabilitiesAccessError;

  return (
    <main className="mx-auto flex min-h-screen w-full max-w-6xl flex-col gap-6 px-4 py-6">
      <header className="rounded-md border border-[--color-border] bg-[--color-bg-surface] px-4 py-3">
        <h1 className="text-2xl font-semibold">{m.app_title()}</h1>
        <p className="text-sm text-[--color-fg-muted]">
          {m.config_page_subtitle()}
        </p>
      </header>

      <SettingsPanel
        accessError={settingsAccessError ?? sharedAccessError}
        busy={busy}
        canDeleteSettings={canDeleteSettings}
        canReadSettings={canReadSettings}
        canWriteSettings={canWriteSettings}
        capabilitiesLoading={capabilitiesLoading}
        onList={listSettings}
        onRemove={removeUserSetting}
        onSet={setUserSetting}
        readModel={settingsReadModel}
      />

      <CredentialsPanel
        accessError={credentialsAccessError ?? sharedAccessError}
        busy={busy}
        canCreateCredentials={canCreateCredentials}
        canDeleteCredentials={canDeleteCredentials}
        canReadCredentials={canReadCredentials}
        canUpdateCredentials={canUpdateCredentials}
        capabilitiesLoading={capabilitiesLoading}
        onAdd={addCredential}
        onList={listCredentials}
        onRemove={removeCredential}
        onUpdate={updateCredential}
        readModel={credentialsReadModel}
      />

      {uiError !== null ? (
        <p className="text-sm text-[--color-error]">{uiError}</p>
      ) : null}
      {uiMessage !== null ? (
        <p className="text-sm text-[--color-success]">{uiMessage}</p>
      ) : null}
    </main>
  );
}
