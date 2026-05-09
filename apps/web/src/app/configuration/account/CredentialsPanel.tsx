import { useState } from "react";
import type { ReactNode } from "react";

import { CredentialSecretForm } from "@/app/configuration/account/CredentialSecretForm";
import type {
  CreateUserCredentialInput,
  ListUserCredentialsResult,
  UpdateUserCredentialInput,
  UserCredentialKind,
} from "@/app/lib/api-contracts";
import * as m from "@/i18n/paraglide/messages";

type CredentialPanel = "list" | "add" | "update" | "remove";

interface CredentialsPanelProps {
  accessError: string | null;
  busy: boolean;
  canCreateCredentials: boolean;
  canDeleteCredentials: boolean;
  canReadCredentials: boolean;
  canUpdateCredentials: boolean;
  capabilitiesLoading: boolean;
  onAdd: (input: CreateUserCredentialInput) => Promise<void>;
  onList: () => Promise<void>;
  onRemove: (itemId: string) => Promise<void>;
  onUpdate: (itemId: string, input: UpdateUserCredentialInput) => Promise<void>;
  readModel: ListUserCredentialsResult | null;
}

export function CredentialsPanel({
  accessError,
  busy,
  canCreateCredentials,
  canDeleteCredentials,
  canReadCredentials,
  canUpdateCredentials,
  capabilitiesLoading,
  onAdd,
  onList,
  onRemove,
  onUpdate,
  readModel,
}: CredentialsPanelProps): ReactNode {
  const [panel, setPanel] = useState<CredentialPanel>("list");
  const [credentialKind, setCredentialKind] =
    useState<UserCredentialKind>("provider_api_token");
  const [credentialItemId, setCredentialItemId] = useState("");

  const rows = readModel?.items ?? [];
  const metadata = [
    { label: "next_cursor", value: readModel?.next_cursor },
    { label: "freshness", value: readModel?.freshness },
    { label: "as_of", value: readModel?.as_of },
    { label: "generated_at", value: readModel?.generated_at },
  ].filter(
    (item): item is { label: string; value: string } =>
      typeof item.value === "string" && item.value.trim() !== "",
  );

  async function submitAdd(secret: string): Promise<void> {
    await onAdd({
      kind: credentialKind,
      value: secret,
    });
  }

  async function submitUpdate(secret: string): Promise<void> {
    const itemId = credentialItemId.trim();
    if (itemId === "") {
      return;
    }
    await onUpdate(itemId, {
      value: secret,
    });
  }

  async function submitRemove(): Promise<void> {
    const itemId = credentialItemId.trim();
    if (itemId === "") {
      return;
    }
    await onRemove(itemId);
  }

  return (
    <section className="rounded-md border border-[--color-border] bg-[--color-bg-surface] p-4">
      <h2 className="mb-3 text-sm font-semibold">
        {m.config_credentials_title()}
      </h2>

      {capabilitiesLoading ? (
        <p className="mb-3 text-sm text-[--color-fg-muted]">
          {m.config_access_loading()}
        </p>
      ) : null}

      {accessError !== null ? (
        <p className="mb-3 text-sm text-[--color-fg-muted]">{accessError}</p>
      ) : null}

      <div className="mb-3 flex flex-wrap gap-2">
        {[
          {
            enabled: canReadCredentials,
            name: "list" as const,
            label: m.config_credentials_panel_list(),
          },
          {
            enabled: canCreateCredentials,
            name: "add" as const,
            label: m.config_credentials_panel_add(),
          },
          {
            enabled: canUpdateCredentials,
            name: "update" as const,
            label: m.config_credentials_panel_update(),
          },
          {
            enabled: canDeleteCredentials,
            name: "remove" as const,
            label: m.config_credentials_panel_remove(),
          },
        ].map((tab) => {
          if (!tab.enabled) {
            return null;
          }
          return (
            <button
              key={tab.name}
              type="button"
              onClick={() => setPanel(tab.name)}
              className={`rounded-md px-3 py-1.5 text-sm ${panel === tab.name ? "bg-[--color-accent] text-[--color-accent-fg]" : "border border-[--color-border]"}`}
            >
              {tab.label}
            </button>
          );
        })}
      </div>

      {panel === "list" && canReadCredentials ? (
        <button
          type="button"
          onClick={() => void onList()}
          disabled={busy}
          className="mb-3 rounded-md bg-[--color-accent] px-3 py-1.5 text-sm text-[--color-accent-fg] disabled:opacity-60"
        >
          {m.config_credentials_list_button()}
        </button>
      ) : null}

      {panel === "add" && canCreateCredentials ? (
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
          <CredentialSecretForm
            busy={busy}
            className="flex min-w-48 flex-1 flex-wrap items-center gap-2"
            placeholder={m.config_credentials_placeholder_secret()}
            submitLabel={m.config_credentials_add_button()}
            onSubmit={submitAdd}
          />
        </div>
      ) : null}

      {panel === "update" && canUpdateCredentials ? (
        <div className="mb-3 flex flex-wrap items-center gap-2">
          <input
            value={credentialItemId}
            onChange={(event) => setCredentialItemId(event.target.value)}
            className="min-w-48 flex-1 rounded-md border border-[--color-border] bg-[--color-bg-canvas] px-3 py-1.5 text-sm"
            placeholder={m.config_credentials_placeholder_item_id()}
          />
          <CredentialSecretForm
            busy={busy}
            className="flex min-w-48 flex-1 flex-wrap items-center gap-2"
            placeholder={m.config_credentials_placeholder_new_secret()}
            submitLabel={m.config_credentials_update_button()}
            onSubmit={submitUpdate}
          />
        </div>
      ) : null}

      {panel === "remove" && canDeleteCredentials ? (
        <div className="mb-4 flex flex-wrap items-center gap-2">
          <input
            value={credentialItemId}
            onChange={(event) => setCredentialItemId(event.target.value)}
            className="min-w-48 flex-1 rounded-md border border-[--color-border] bg-[--color-bg-canvas] px-3 py-1.5 text-sm"
            placeholder={m.config_credentials_placeholder_item_id()}
          />
          <button
            type="button"
            onClick={() => void submitRemove()}
            disabled={busy}
            className="rounded-md border border-[--color-border] px-3 py-1.5 text-sm disabled:opacity-60"
          >
            {m.config_credentials_remove_button()}
          </button>
        </div>
      ) : null}

      <div className="mb-3 rounded-md border border-[--color-border] bg-[--color-bg-canvas] p-3 text-xs">
        <p className="m-0">rows={rows.length}</p>
        {metadata.map((item) => (
          <p key={item.label} className="m-0">
            {item.label}={item.value}
          </p>
        ))}
      </div>

      {rows.length === 0 ? (
        <p className="m-0 text-sm text-[--color-fg-muted]">
          No credential metadata returned.
        </p>
      ) : (
        <div className="overflow-x-auto rounded-md border border-[--color-border]">
          <table className="w-full border-collapse text-left text-xs">
            <thead className="bg-[--color-bg-canvas]">
              <tr>
                <th className="border-b border-[--color-border] px-3 py-2">
                  id
                </th>
                <th className="border-b border-[--color-border] px-3 py-2">
                  kind
                </th>
                <th className="border-b border-[--color-border] px-3 py-2">
                  status
                </th>
                <th className="border-b border-[--color-border] px-3 py-2">
                  owner_scope
                </th>
                <th className="border-b border-[--color-border] px-3 py-2">
                  created_at
                </th>
                <th className="border-b border-[--color-border] px-3 py-2">
                  updated_at
                </th>
                <th className="border-b border-[--color-border] px-3 py-2">
                  {m.config_credentials_redaction_state_label()}
                </th>
              </tr>
            </thead>
            <tbody>
              {rows.map((credential) => (
                <tr key={credential.id}>
                  <td className="border-b border-[--color-border] px-3 py-2">
                    {credential.id}
                  </td>
                  <td className="border-b border-[--color-border] px-3 py-2">
                    {credential.kind}
                  </td>
                  <td className="border-b border-[--color-border] px-3 py-2">
                    {credential.status}
                  </td>
                  <td className="border-b border-[--color-border] px-3 py-2">
                    {`${credential.owner_scope.scope}:${credential.owner_scope.account_id}`}
                  </td>
                  <td className="border-b border-[--color-border] px-3 py-2">
                    {credential.created_at}
                  </td>
                  <td className="border-b border-[--color-border] px-3 py-2">
                    {credential.updated_at}
                  </td>
                  <td className="border-b border-[--color-border] px-3 py-2">
                    {m.config_credentials_redaction_state_redacted()}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </section>
  );
}
