import type { ReactNode } from "react";

import { CredentialSecretForm } from "@/app/configuration/account/CredentialSecretForm";
import type { CredentialKindDescriptor } from "@/app/configuration/account/credentialKinds";
import { toUserCredentialKind } from "@/app/configuration/account/credentialKinds";
import type {
  SecretInput,
  UserCredentialItemId,
  UserCredentialKind,
} from "@/app/lib/api-contracts";
import * as m from "@/i18n/paraglide/messages";

export interface CredentialSelectableRow {
  itemId: UserCredentialItemId;
  label: string;
}

interface CredentialRowSelectProps {
  selectedCredentialId: UserCredentialItemId | null;
  selectableRows: readonly CredentialSelectableRow[];
  onSelectCredential: (itemId: UserCredentialItemId | null) => void;
}

function CredentialRowSelect({
  selectedCredentialId,
  selectableRows,
  onSelectCredential,
}: CredentialRowSelectProps): ReactNode {
  return (
    <select
      value={selectedCredentialId ?? ""}
      onChange={(event) => {
        const selectedRow = selectableRows.find(
          (row) => row.itemId === event.target.value,
        );
        onSelectCredential(selectedRow?.itemId ?? null);
      }}
      className="min-w-72 flex-1 rounded-md border border-[--color-border] bg-[--color-bg-canvas] px-3 py-1.5 text-sm"
    >
      <option value="">{m.config_credentials_select_row_placeholder()}</option>
      {selectableRows.map((row) => (
        <option key={row.itemId} value={row.itemId}>
          {row.label}
        </option>
      ))}
    </select>
  );
}

interface AddCredentialFormProps {
  busy: boolean;
  credentialKind: UserCredentialKind;
  kindDescriptors: readonly CredentialKindDescriptor[];
  onCredentialKindChange: (kind: UserCredentialKind) => void;
  onSubmit: (secret: SecretInput) => Promise<void>;
}

export function AddCredentialForm({
  busy,
  credentialKind,
  kindDescriptors,
  onCredentialKindChange,
  onSubmit,
}: AddCredentialFormProps): ReactNode {
  return (
    <div className="mb-3 flex flex-wrap items-center gap-2">
      <select
        value={credentialKind}
        onChange={(event) => {
          const kind = toUserCredentialKind(event.target.value);
          if (kind !== null) {
            onCredentialKindChange(kind);
          }
        }}
        className="rounded-md border border-[--color-border] bg-[--color-bg-canvas] px-2 py-1.5 text-sm"
      >
        {kindDescriptors.map((descriptor) => (
          <option key={descriptor.kind} value={descriptor.kind}>
            {descriptor.label()}
          </option>
        ))}
      </select>
      <CredentialSecretForm
        busy={busy}
        className="flex min-w-48 flex-1 flex-wrap items-center gap-2"
        placeholder={m.config_credentials_placeholder_secret()}
        submitLabel={m.config_credentials_add_button()}
        onSubmit={onSubmit}
      />
    </div>
  );
}

interface UpdateCredentialFormProps {
  busy: boolean;
  selectedCredentialId: UserCredentialItemId | null;
  selectableRows: readonly CredentialSelectableRow[];
  onSelectCredential: (itemId: UserCredentialItemId | null) => void;
  onSubmit: (secret: SecretInput) => Promise<void>;
}

export function UpdateCredentialForm({
  busy,
  selectedCredentialId,
  selectableRows,
  onSelectCredential,
  onSubmit,
}: UpdateCredentialFormProps): ReactNode {
  return (
    <div className="mb-3 flex flex-wrap items-center gap-2">
      <CredentialRowSelect
        selectedCredentialId={selectedCredentialId}
        selectableRows={selectableRows}
        onSelectCredential={onSelectCredential}
      />
      <CredentialSecretForm
        busy={busy || selectedCredentialId === null}
        className="flex min-w-48 flex-1 flex-wrap items-center gap-2"
        placeholder={m.config_credentials_placeholder_new_secret()}
        submitLabel={m.config_credentials_update_button()}
        onSubmit={onSubmit}
      />
    </div>
  );
}

interface RemoveCredentialFormProps {
  busy: boolean;
  selectedCredentialId: UserCredentialItemId | null;
  selectableRows: readonly CredentialSelectableRow[];
  onSelectCredential: (itemId: UserCredentialItemId | null) => void;
  onSubmit: () => Promise<void>;
}

export function RemoveCredentialForm({
  busy,
  selectedCredentialId,
  selectableRows,
  onSelectCredential,
  onSubmit,
}: RemoveCredentialFormProps): ReactNode {
  return (
    <div className="mb-4 flex flex-wrap items-center gap-2">
      <CredentialRowSelect
        selectedCredentialId={selectedCredentialId}
        selectableRows={selectableRows}
        onSelectCredential={onSelectCredential}
      />
      <button
        type="button"
        onClick={() => void onSubmit()}
        disabled={busy || selectedCredentialId === null}
        className="rounded-md border border-[--color-border] px-3 py-1.5 text-sm disabled:opacity-60"
      >
        {m.config_credentials_remove_button()}
      </button>
    </div>
  );
}
