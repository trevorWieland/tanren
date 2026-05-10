"use client";

import { useEffect, useState } from "react";

import {
  addUserCredential,
  listUserCredentials,
  removeUserCredential as removeUserCredentialApi,
  updateUserCredential as updateUserCredentialApi,
} from "@/app/lib/account-client";
import type {
  CredentialListPageInput,
  CreateUserCredentialInput,
  ListUserCredentialsResult,
  UpdateUserCredentialInput,
  UserCredentialItemId,
} from "@/app/lib/api-contracts";
import * as m from "@/i18n/paraglide/messages";

import { formatRequestError } from "@/app/configuration/account/configurationRequestErrors";

const DEFAULT_CREDENTIAL_PAGE_SIZE = 20;

export interface CredentialsFeedbackSink {
  setUiError: (value: string | null) => void;
  setUiMessage: (value: string | null) => void;
}

export interface UseCredentialsModelInput {
  discoveredAccessError: string | null;
  discoveredReadModel: ListUserCredentialsResult | null;
  feedback: CredentialsFeedbackSink;
}

export interface CredentialsModel {
  accessError: string | null;
  addCredential: (input: CreateUserCredentialInput) => Promise<void>;
  busy: boolean;
  listCredentials: (input: CredentialListPageInput) => Promise<void>;
  readModel: ListUserCredentialsResult | null;
  removeCredential: (itemId: UserCredentialItemId) => Promise<void>;
  updateCredential: (
    itemId: UserCredentialItemId,
    input: UpdateUserCredentialInput,
  ) => Promise<void>;
}

function normalizeCredentialListInput(
  input: CredentialListPageInput,
  fallbackPageSize: number,
): CredentialListPageInput {
  const normalizedPageSize =
    typeof input.page_size === "number" ? input.page_size : fallbackPageSize;

  if (typeof input.cursor === "string" && input.cursor !== "") {
    return {
      cursor: input.cursor,
      page_size: normalizedPageSize,
    };
  }

  return {
    page_size: normalizedPageSize,
  };
}

export function useCredentialsModel(
  input: UseCredentialsModelInput,
): CredentialsModel {
  const [busy, setBusy] = useState(false);
  const [accessError, setAccessError] = useState<string | null>(
    input.discoveredAccessError,
  );
  const [readModel, setReadModel] = useState<ListUserCredentialsResult | null>(
    input.discoveredReadModel,
  );
  const [credentialListInput, setCredentialListInput] =
    useState<CredentialListPageInput>({
      page_size: DEFAULT_CREDENTIAL_PAGE_SIZE,
    });

  useEffect(() => {
    setReadModel(input.discoveredReadModel);
  }, [input.discoveredReadModel]);

  useEffect(() => {
    setAccessError(input.discoveredAccessError);
  }, [input.discoveredAccessError]);

  async function refreshCredentialsReadModel(
    nextInput: CredentialListPageInput = credentialListInput,
  ): Promise<void> {
    const body = await listUserCredentials(nextInput);
    setReadModel(body);
    setAccessError(null);
  }

  async function listCredentials(
    inputValue: CredentialListPageInput,
  ): Promise<void> {
    const normalizedInput = normalizeCredentialListInput(
      inputValue,
      credentialListInput.page_size ?? DEFAULT_CREDENTIAL_PAGE_SIZE,
    );

    setBusy(true);
    input.feedback.setUiError(null);
    input.feedback.setUiMessage(null);
    try {
      await refreshCredentialsReadModel(normalizedInput);
      setCredentialListInput(normalizedInput);
      input.feedback.setUiMessage(m.config_credentials_list_success());
    } catch (error: unknown) {
      input.feedback.setUiError(formatRequestError(error));
    } finally {
      setBusy(false);
    }
  }

  async function addCredential(
    inputValue: CreateUserCredentialInput,
  ): Promise<void> {
    setBusy(true);
    input.feedback.setUiError(null);
    input.feedback.setUiMessage(null);
    try {
      await addUserCredential(inputValue);
      await refreshCredentialsReadModel(credentialListInput);
      input.feedback.setUiMessage(m.config_credentials_add_success());
    } catch (error: unknown) {
      input.feedback.setUiError(formatRequestError(error));
    } finally {
      setBusy(false);
    }
  }

  async function updateCredential(
    itemId: UserCredentialItemId,
    inputValue: UpdateUserCredentialInput,
  ): Promise<void> {
    setBusy(true);
    input.feedback.setUiError(null);
    input.feedback.setUiMessage(null);
    try {
      await updateUserCredentialApi(itemId, inputValue);
      await refreshCredentialsReadModel(credentialListInput);
      input.feedback.setUiMessage(m.config_credentials_update_success());
    } catch (error: unknown) {
      input.feedback.setUiError(formatRequestError(error));
    } finally {
      setBusy(false);
    }
  }

  async function removeCredential(itemId: UserCredentialItemId): Promise<void> {
    setBusy(true);
    input.feedback.setUiError(null);
    input.feedback.setUiMessage(null);
    try {
      await removeUserCredentialApi(itemId);
      await refreshCredentialsReadModel(credentialListInput);
      input.feedback.setUiMessage(m.config_credentials_remove_success());
    } catch (error: unknown) {
      input.feedback.setUiError(formatRequestError(error));
    } finally {
      setBusy(false);
    }
  }

  return {
    accessError,
    addCredential,
    busy,
    listCredentials,
    readModel,
    removeCredential,
    updateCredential,
  };
}
