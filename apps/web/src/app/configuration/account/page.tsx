"use client";

import { useMemo, useState } from "react";
import type { ReactNode } from "react";

import { CredentialsPanel } from "@/app/configuration/account/CredentialsPanel";
import { SettingsPanel } from "@/app/configuration/account/SettingsPanel";
import { useConfigurationAccess } from "@/app/configuration/account/useConfigurationAccess";
import { useCredentialsModel } from "@/app/configuration/account/useCredentialsModel";
import { useSettingsModel } from "@/app/configuration/account/useSettingsModel";
import * as m from "@/i18n/paraglide/messages";

export default function ConfigurationAccountPage(): ReactNode {
  const [uiError, setUiError] = useState<string | null>(null);
  const [uiMessage, setUiMessage] = useState<string | null>(null);

  const accessModel = useConfigurationAccess();
  const feedback = useMemo(
    () => ({
      setUiError,
      setUiMessage,
    }),
    [],
  );

  const settingsModel = useSettingsModel({
    discoveredAccessError: accessModel.settingsAccessError,
    discoveredReadModel: accessModel.discoveredSettingsReadModel,
    feedback,
  });
  const credentialsModel = useCredentialsModel({
    discoveredAccessError: accessModel.userCredentialsAccessError,
    discoveredReadModel: accessModel.discoveredCredentialsReadModel,
    feedback,
  });

  const busy = settingsModel.busy || credentialsModel.busy;
  const sharedAccessError = accessModel.capabilitiesAccessError;

  return (
    <main className="mx-auto flex min-h-screen w-full max-w-6xl flex-col gap-6 px-4 py-6">
      <header className="rounded-md border border-[--color-border] bg-[--color-bg-surface] px-4 py-3">
        <h1 className="text-2xl font-semibold">{m.app_title()}</h1>
        <p className="text-sm text-[--color-fg-muted]">
          {m.config_page_subtitle()}
        </p>
      </header>

      <SettingsPanel
        accessError={settingsModel.accessError ?? sharedAccessError}
        busy={busy}
        canDeleteSettings={accessModel.canDeleteSettings}
        canReadSettings={accessModel.canReadSettings}
        canWriteSettings={accessModel.canWriteSettings}
        capabilitiesLoading={accessModel.capabilitiesLoading}
        onList={settingsModel.listSettings}
        onRemove={settingsModel.removeUserSetting}
        onSet={settingsModel.setUserSetting}
        readModel={settingsModel.readModel}
      />

      <CredentialsPanel
        accessError={credentialsModel.accessError ?? sharedAccessError}
        busy={busy}
        canCreateCredentials={accessModel.canCreateCredentials}
        canDeleteCredentials={accessModel.canDeleteCredentials}
        canReadCredentials={accessModel.canReadCredentials}
        canUpdateCredentials={accessModel.canUpdateCredentials}
        capabilitiesLoading={accessModel.capabilitiesLoading}
        onAdd={credentialsModel.addCredential}
        onList={credentialsModel.listCredentials}
        onRemove={credentialsModel.removeCredential}
        onUpdate={credentialsModel.updateCredential}
        readModel={credentialsModel.readModel}
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
