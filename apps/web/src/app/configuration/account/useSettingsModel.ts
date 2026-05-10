"use client";

import { useEffect, useState } from "react";

import {
  listUserSettings,
  removeUserSetting as removeUserSettingApi,
  upsertUserSetting,
} from "@/app/lib/account-client";
import type {
  ListUserSettingsResult,
  UpsertUserSettingInput,
} from "@/app/lib/api-contracts";
import * as m from "@/i18n/paraglide/messages";

import { formatRequestError } from "@/app/configuration/account/configurationRequestErrors";

export interface SettingsFeedbackSink {
  setUiError: (value: string | null) => void;
  setUiMessage: (value: string | null) => void;
}

export interface UseSettingsModelInput {
  discoveredAccessError: string | null;
  discoveredReadModel: ListUserSettingsResult | null;
  feedback: SettingsFeedbackSink;
}

export interface SettingsModel {
  accessError: string | null;
  busy: boolean;
  listSettings: () => Promise<void>;
  readModel: ListUserSettingsResult | null;
  removeUserSetting: (key: UpsertUserSettingInput["key"]) => Promise<void>;
  setUserSetting: (input: UpsertUserSettingInput) => Promise<void>;
}

export function useSettingsModel(input: UseSettingsModelInput): SettingsModel {
  const [busy, setBusy] = useState(false);
  const [accessError, setAccessError] = useState<string | null>(
    input.discoveredAccessError,
  );
  const [readModel, setReadModel] = useState<ListUserSettingsResult | null>(
    input.discoveredReadModel,
  );

  useEffect(() => {
    setReadModel(input.discoveredReadModel);
  }, [input.discoveredReadModel]);

  useEffect(() => {
    setAccessError(input.discoveredAccessError);
  }, [input.discoveredAccessError]);

  async function refreshSettingsReadModel(): Promise<void> {
    const body = await listUserSettings();
    setReadModel(body);
    setAccessError(null);
  }

  async function listSettings(): Promise<void> {
    setBusy(true);
    input.feedback.setUiError(null);
    input.feedback.setUiMessage(null);
    try {
      await refreshSettingsReadModel();
      input.feedback.setUiMessage(m.config_settings_list_success());
    } catch (error: unknown) {
      input.feedback.setUiError(formatRequestError(error));
    } finally {
      setBusy(false);
    }
  }

  async function setUserSetting(
    inputValue: UpsertUserSettingInput,
  ): Promise<void> {
    setBusy(true);
    input.feedback.setUiError(null);
    input.feedback.setUiMessage(null);
    try {
      await upsertUserSetting(inputValue);
      await refreshSettingsReadModel();
      input.feedback.setUiMessage(m.config_settings_set_success());
    } catch (error: unknown) {
      input.feedback.setUiError(formatRequestError(error));
    } finally {
      setBusy(false);
    }
  }

  async function removeUserSetting(
    key: UpsertUserSettingInput["key"],
  ): Promise<void> {
    setBusy(true);
    input.feedback.setUiError(null);
    input.feedback.setUiMessage(null);
    try {
      await removeUserSettingApi(key);
      await refreshSettingsReadModel();
      input.feedback.setUiMessage(m.config_settings_remove_success());
    } catch (error: unknown) {
      input.feedback.setUiError(formatRequestError(error));
    } finally {
      setBusy(false);
    }
  }

  return {
    accessError,
    busy,
    listSettings,
    readModel,
    removeUserSetting,
    setUserSetting,
  };
}
