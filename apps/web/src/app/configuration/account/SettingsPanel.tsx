import { useState } from "react";
import type { ReactNode } from "react";

import {
  USER_SETTING_DESCRIPTORS,
  userSettingDescriptorForKey,
} from "@/app/configuration/account/settingDescriptors";
import type {
  ListUserSettingsResult,
  UserSettingKey,
  UpsertUserSettingInput,
} from "@/app/lib/api-contracts";
import * as m from "@/i18n/paraglide/messages";

interface SettingsPanelProps {
  accessError: string | null;
  busy: boolean;
  canDeleteSettings: boolean;
  canReadSettings: boolean;
  canWriteSettings: boolean;
  capabilitiesLoading: boolean;
  onList: () => Promise<void>;
  onRemove: (key: UserSettingKey) => Promise<void>;
  onSet: (input: UpsertUserSettingInput) => Promise<void>;
  readModel: ListUserSettingsResult | null;
}

function renderSettingValue(value: UpsertUserSettingInput["value"]): string {
  return value.value;
}

export function SettingsPanel({
  accessError,
  busy,
  canDeleteSettings,
  canReadSettings,
  canWriteSettings,
  capabilitiesLoading,
  onList,
  onRemove,
  onSet,
  readModel,
}: SettingsPanelProps): ReactNode {
  const [settingKey, setSettingKey] = useState<UserSettingKey>(
    USER_SETTING_DESCRIPTORS[0]?.key ?? "theme",
  );
  const [settingValue, setSettingValue] = useState("system");
  const [removeSettingKey, setRemoveSettingKey] = useState<UserSettingKey>(
    USER_SETTING_DESCRIPTORS[0]?.key ?? "theme",
  );

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
  const selectedDescriptor = userSettingDescriptorForKey(settingKey);

  async function submitSet(): Promise<void> {
    const rawValue = settingValue.trim();
    if (rawValue === "") {
      return;
    }

    const valuePayload = selectedDescriptor.toValue(rawValue);
    if (valuePayload === null) {
      return;
    }

    await onSet({
      key: settingKey,
      value: valuePayload,
    });
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
        {m.config_settings_title()}
      </h2>

      {capabilitiesLoading ? (
        <p className="mb-3 text-sm text-[--color-fg-muted]">
          {m.config_access_loading()}
        </p>
      ) : null}

      {accessError !== null ? (
        <p className="mb-3 text-sm text-[--color-fg-muted]">{accessError}</p>
      ) : null}

      <div className="mb-3">
        {canReadSettings ? (
          <button
            type="button"
            onClick={() => void onList()}
            disabled={busy}
            className="rounded-md bg-[--color-accent] px-3 py-1.5 text-sm text-[--color-accent-fg] disabled:opacity-60"
          >
            {m.config_settings_list_button()}
          </button>
        ) : (
          unavailable(m.config_settings_capability_required_read())
        )}
      </div>

      <div className="mb-3">
        {canWriteSettings ? (
          <div className="flex flex-wrap items-center gap-2">
            <select
              value={settingKey}
              onChange={(event) =>
                setSettingKey(event.target.value as UserSettingKey)
              }
              className="rounded-md border border-[--color-border] bg-[--color-bg-canvas] px-2 py-1.5 text-sm"
            >
              {USER_SETTING_DESCRIPTORS.map((descriptor) => (
                <option key={descriptor.key} value={descriptor.key}>
                  {descriptor.label()}
                </option>
              ))}
            </select>
            <input
              value={settingValue}
              onChange={(event) => setSettingValue(event.target.value)}
              className="min-w-48 flex-1 rounded-md border border-[--color-border] bg-[--color-bg-canvas] px-3 py-1.5 text-sm"
              placeholder={selectedDescriptor.placeholder()}
            />
            <button
              type="button"
              onClick={() => void submitSet()}
              disabled={busy}
              className="rounded-md bg-[--color-accent] px-3 py-1.5 text-sm text-[--color-accent-fg] disabled:opacity-60"
            >
              {m.config_settings_set_button()}
            </button>
          </div>
        ) : (
          unavailable(m.config_settings_capability_required_write())
        )}
      </div>

      <div className="mb-4">
        {canDeleteSettings ? (
          <div className="flex flex-wrap items-center gap-2">
            <select
              value={removeSettingKey}
              onChange={(event) =>
                setRemoveSettingKey(event.target.value as UserSettingKey)
              }
              className="rounded-md border border-[--color-border] bg-[--color-bg-canvas] px-2 py-1.5 text-sm"
            >
              {USER_SETTING_DESCRIPTORS.map((descriptor) => (
                <option key={descriptor.key} value={descriptor.key}>
                  {descriptor.label()}
                </option>
              ))}
            </select>
            <button
              type="button"
              onClick={() => void onRemove(removeSettingKey)}
              disabled={busy}
              className="rounded-md border border-[--color-border] px-3 py-1.5 text-sm disabled:opacity-60"
            >
              {m.config_settings_remove_button()}
            </button>
          </div>
        ) : (
          unavailable(m.config_settings_capability_required_delete())
        )}
      </div>

      {canReadSettings ? (
        <>
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
              No settings returned.
            </p>
          ) : (
            <div className="overflow-x-auto rounded-md border border-[--color-border]">
              <table className="w-full border-collapse text-left text-xs">
                <thead className="bg-[--color-bg-canvas]">
                  <tr>
                    <th className="border-b border-[--color-border] px-3 py-2">
                      key
                    </th>
                    <th className="border-b border-[--color-border] px-3 py-2">
                      kind
                    </th>
                    <th className="border-b border-[--color-border] px-3 py-2">
                      value
                    </th>
                    <th className="border-b border-[--color-border] px-3 py-2">
                      updated_at
                    </th>
                  </tr>
                </thead>
                <tbody>
                  {rows.map((setting) => (
                    <tr key={setting.key}>
                      <td className="border-b border-[--color-border] px-3 py-2">
                        {setting.key}
                      </td>
                      <td className="border-b border-[--color-border] px-3 py-2">
                        {setting.value.kind}
                      </td>
                      <td className="border-b border-[--color-border] px-3 py-2">
                        {renderSettingValue(setting.value)}
                      </td>
                      <td className="border-b border-[--color-border] px-3 py-2">
                        {setting.updated_at}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </>
      ) : (
        <div className="rounded-md border border-[--color-border] bg-[--color-bg-canvas] p-3 text-xs text-[--color-fg-muted]">
          {m.config_settings_capability_required_read()}
        </div>
      )}
    </section>
  );
}
