import { useEffect, useState } from "react";
import type { ReactNode } from "react";

import { CredentialSecretForm } from "@/app/configuration/account/CredentialSecretForm";
import type {
  CredentialListPageInput,
  CreateUserCredentialInput,
  ListUserCredentialsResult,
  SecretInput,
  UpdateUserCredentialInput,
  UserCredentialItemId,
  UserCredentialKind,
  UserCredentialView,
} from "@/app/lib/api-contracts";
import { userCredentialItemId } from "@/app/lib/api-contracts";
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
  onList: (input: CredentialListPageInput) => Promise<void>;
  onRemove: (itemId: UserCredentialItemId) => Promise<void>;
  onUpdate: (
    itemId: UserCredentialItemId,
    input: UpdateUserCredentialInput,
  ) => Promise<void>;
  readModel: ListUserCredentialsResult | null;
}

const DEFAULT_PAGE_SIZE = 20;

function renderCredentialRowLabel(
  credential: UserCredentialView,
  index: number,
): string {
  return `${index + 1}. ${credential.kind} · ${credential.status} · ${credential.updated_at}`;
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
  const [selectedCredentialId, setSelectedCredentialId] =
    useState<UserCredentialItemId | null>(null);
  const [pageSize, setPageSize] = useState(DEFAULT_PAGE_SIZE);
  const [pageCursors, setPageCursors] = useState<Array<string | null>>([null]);

  const rows = readModel?.items ?? [];
  const nextCursor =
    typeof readModel?.next_cursor === "string" &&
    readModel.next_cursor.trim() !== ""
      ? readModel.next_cursor
      : null;
  const hasPreviousPage = pageCursors.length > 1;
  const metadata = [
    { label: "rows", value: String(rows.length) },
    { label: "freshness", value: readModel?.freshness },
    { label: "as_of", value: readModel?.as_of },
    { label: "generated_at", value: readModel?.generated_at },
  ].filter(
    (item): item is { label: string; value: string } =>
      typeof item.value === "string" && item.value.trim() !== "",
  );

  const selectableRows = rows.map((credential, index) => ({
    itemId: userCredentialItemId(credential.id),
    label: renderCredentialRowLabel(credential, index),
  }));

  useEffect(() => {
    if (
      selectedCredentialId !== null &&
      !selectableRows.some((row) => row.itemId === selectedCredentialId)
    ) {
      setSelectedCredentialId(null);
    }
  }, [selectableRows, selectedCredentialId]);

  async function submitAdd(secret: SecretInput): Promise<void> {
    await onAdd({
      kind: credentialKind,
      value: secret.value,
    });
  }

  async function submitUpdate(secret: SecretInput): Promise<void> {
    if (selectedCredentialId === null) {
      return;
    }
    await onUpdate(selectedCredentialId, {
      value: secret.value,
    });
  }

  async function submitRemove(): Promise<void> {
    if (selectedCredentialId === null) {
      return;
    }
    await onRemove(selectedCredentialId);
  }

  async function loadFirstPage(): Promise<void> {
    await onList({ page_size: pageSize });
    setPageCursors([null]);
  }

  async function loadNextPage(): Promise<void> {
    if (nextCursor === null) {
      return;
    }
    await onList({
      cursor: nextCursor,
      page_size: pageSize,
    });
    setPageCursors((previous) => [...previous, nextCursor]);
  }

  async function loadPreviousPage(): Promise<void> {
    if (!hasPreviousPage) {
      return;
    }
    const previousCursor = pageCursors[pageCursors.length - 2] ?? null;
    const request: CredentialListPageInput =
      previousCursor === null
        ? { page_size: pageSize }
        : { cursor: previousCursor, page_size: pageSize };
    await onList(request);
    setPageCursors((previous) => previous.slice(0, -1));
  }

  function unavailable(message: string): ReactNode {
    return (
      <p className="m-0 rounded-md border border-dashed border-[--color-border] bg-[--color-bg-canvas] px-3 py-2 text-sm text-[--color-fg-muted]">
        {message}
      </p>
    );
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

      {panel === "list" ? (
        canReadCredentials ? (
          <div className="mb-4 flex flex-wrap items-center gap-2">
            <label className="text-sm" htmlFor="credential-page-size">
              {m.config_credentials_page_size_label()}
            </label>
            <select
              id="credential-page-size"
              value={String(pageSize)}
              onChange={(event) =>
                setPageSize(Number.parseInt(event.target.value, 10))
              }
              className="rounded-md border border-[--color-border] bg-[--color-bg-canvas] px-2 py-1.5 text-sm"
            >
              {[10, 20, 50, 100].map((size) => (
                <option key={size} value={String(size)}>
                  {size}
                </option>
              ))}
            </select>
            <button
              type="button"
              onClick={() => void loadFirstPage()}
              disabled={busy}
              className="rounded-md bg-[--color-accent] px-3 py-1.5 text-sm text-[--color-accent-fg] disabled:opacity-60"
            >
              {m.config_credentials_list_button()}
            </button>
            <button
              type="button"
              onClick={() => void loadPreviousPage()}
              disabled={busy || !hasPreviousPage}
              className="rounded-md border border-[--color-border] px-3 py-1.5 text-sm disabled:opacity-60"
            >
              {m.config_credentials_previous_page_button()}
            </button>
            {nextCursor !== null ? (
              <button
                type="button"
                onClick={() => void loadNextPage()}
                disabled={busy}
                className="rounded-md border border-[--color-border] px-3 py-1.5 text-sm disabled:opacity-60"
              >
                {m.config_credentials_next_page_button()}
              </button>
            ) : null}
          </div>
        ) : (
          <div className="mb-4">
            {unavailable(m.config_credentials_capability_required_read())}
          </div>
        )
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

      {panel === "update" ? (
        canUpdateCredentials ? (
          rows.length === 0 ? (
            <div className="mb-3">
              {unavailable(
                m.config_credentials_selection_requires_loaded_rows(),
              )}
            </div>
          ) : (
            <div className="mb-3 flex flex-wrap items-center gap-2">
              <select
                value={selectedCredentialId ?? ""}
                onChange={(event) => {
                  const selected = selectableRows.find(
                    (row) => row.itemId === event.target.value,
                  );
                  setSelectedCredentialId(selected?.itemId ?? null);
                }}
                className="min-w-72 flex-1 rounded-md border border-[--color-border] bg-[--color-bg-canvas] px-3 py-1.5 text-sm"
              >
                <option value="">
                  {m.config_credentials_select_row_placeholder()}
                </option>
                {selectableRows.map((row) => (
                  <option key={row.itemId} value={row.itemId}>
                    {row.label}
                  </option>
                ))}
              </select>
              <CredentialSecretForm
                busy={busy || selectedCredentialId === null}
                className="flex min-w-48 flex-1 flex-wrap items-center gap-2"
                placeholder={m.config_credentials_placeholder_new_secret()}
                submitLabel={m.config_credentials_update_button()}
                onSubmit={submitUpdate}
              />
            </div>
          )
        ) : (
          <div className="mb-3">
            {unavailable(m.config_credentials_capability_required_update())}
          </div>
        )
      ) : null}

      {panel === "remove" ? (
        canDeleteCredentials ? (
          rows.length === 0 ? (
            <div className="mb-4">
              {unavailable(
                m.config_credentials_selection_requires_loaded_rows(),
              )}
            </div>
          ) : (
            <div className="mb-4 flex flex-wrap items-center gap-2">
              <select
                value={selectedCredentialId ?? ""}
                onChange={(event) => {
                  const selected = selectableRows.find(
                    (row) => row.itemId === event.target.value,
                  );
                  setSelectedCredentialId(selected?.itemId ?? null);
                }}
                className="min-w-72 flex-1 rounded-md border border-[--color-border] bg-[--color-bg-canvas] px-3 py-1.5 text-sm"
              >
                <option value="">
                  {m.config_credentials_select_row_placeholder()}
                </option>
                {selectableRows.map((row) => (
                  <option key={row.itemId} value={row.itemId}>
                    {row.label}
                  </option>
                ))}
              </select>
              <button
                type="button"
                onClick={() => void submitRemove()}
                disabled={busy || selectedCredentialId === null}
                className="rounded-md border border-[--color-border] px-3 py-1.5 text-sm disabled:opacity-60"
              >
                {m.config_credentials_remove_button()}
              </button>
            </div>
          )
        ) : (
          <div className="mb-4">
            {unavailable(m.config_credentials_capability_required_delete())}
          </div>
        )
      ) : null}

      {canReadCredentials ? (
        <>
          <div className="mb-3 rounded-md border border-[--color-border] bg-[--color-bg-canvas] p-3 text-xs">
            {metadata.map((item) => (
              <p key={item.label} className="m-0">
                {item.label}={item.value}
              </p>
            ))}
          </div>

          {panel === "list" && busy ? (
            <p className="m-0 text-sm text-[--color-fg-muted]">
              {m.config_credentials_loading_page()}
            </p>
          ) : readModel === null ? (
            <p className="m-0 text-sm text-[--color-fg-muted]">
              {m.config_credentials_not_loaded()}
            </p>
          ) : rows.length === 0 ? (
            <p className="m-0 text-sm text-[--color-fg-muted]">
              {m.config_credentials_empty_page()}
            </p>
          ) : (
            <>
              <p className="mb-2 mt-0 text-xs text-[--color-fg-muted]">
                {hasPreviousPage
                  ? m.config_credentials_boundary_middle_or_last()
                  : m.config_credentials_boundary_first_page()}
                {nextCursor === null
                  ? ` ${m.config_credentials_boundary_last_page()}`
                  : ""}
              </p>
              <div className="overflow-x-auto rounded-md border border-[--color-border]">
                <table className="w-full border-collapse text-left text-xs">
                  <thead className="bg-[--color-bg-canvas]">
                    <tr>
                      <th className="border-b border-[--color-border] px-3 py-2">
                        {m.config_credentials_table_row_label()}
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
                    {rows.map((credential, index) => (
                      <tr key={credential.id}>
                        <td className="border-b border-[--color-border] px-3 py-2">
                          {index + 1}
                        </td>
                        <td className="border-b border-[--color-border] px-3 py-2">
                          {credential.kind}
                        </td>
                        <td className="border-b border-[--color-border] px-3 py-2">
                          {credential.status}
                        </td>
                        <td className="border-b border-[--color-border] px-3 py-2">
                          {credential.owner_scope.scope}
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
            </>
          )}
        </>
      ) : (
        <div className="rounded-md border border-[--color-border] bg-[--color-bg-canvas] p-3 text-xs text-[--color-fg-muted]">
          {m.config_credentials_capability_required_read()}
        </div>
      )}
    </section>
  );
}
