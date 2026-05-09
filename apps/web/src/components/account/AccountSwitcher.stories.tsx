import type { Meta, StoryObj } from "@storybook/nextjs-vite";
import { expect, userEvent, waitFor, within } from "storybook/test";

import { AccountSwitcher } from "./AccountSwitcher";

const activeId = "00000000-0000-7000-8000-000000000010";
const secondId = "00000000-0000-7000-8000-000000000011";

const meta = {
  title: "Account/AccountSwitcher",
  component: AccountSwitcher,
  parameters: {
    a11y: { test: "error" },
    layout: "centered",
  },
} satisfies Meta<typeof AccountSwitcher>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
  play: async ({ canvasElement }) => {
    const c = within(canvasElement);
    const original = globalThis.fetch;
    globalThis.fetch = (async (input, init) => {
      const url = String(input);
      const method = init?.method ?? "GET";
      if (url.endsWith("/accounts/active") && method === "GET") {
        return new Response(
          JSON.stringify({
            accounts: [
              {
                is_active: true,
                account: {
                  id: activeId,
                  identifier: "alice@example.com",
                  display_name: "Alice",
                  org: null,
                },
              },
              {
                is_active: false,
                account: {
                  id: secondId,
                  identifier: "alice@work.example",
                  display_name: "Alice Work",
                  org: "00000000-0000-7000-8000-000000000020",
                },
              },
            ],
          }),
          { status: 200, headers: { "content-type": "application/json" } },
        );
      }
      if (url.endsWith("/accounts/active/switch") && method === "POST") {
        return new Response(
          JSON.stringify({
            active_account_id: secondId,
            accounts: [
              {
                is_active: false,
                account: {
                  id: activeId,
                  identifier: "alice@example.com",
                  display_name: "Alice",
                  org: null,
                },
              },
              {
                is_active: true,
                account: {
                  id: secondId,
                  identifier: "alice@work.example",
                  display_name: "Alice Work",
                  org: "00000000-0000-7000-8000-000000000020",
                },
              },
            ],
          }),
          { status: 200, headers: { "content-type": "application/json" } },
        );
      }
      return new Response(null, { status: 404 });
    }) as typeof fetch;

    try {
      const select = await c.findByRole("combobox");
      await expect(select).toBeVisible();
      await userEvent.selectOptions(select, secondId);
      await waitFor(() => {
        expect((select as HTMLSelectElement).value).toBe(secondId);
      });
    } finally {
      globalThis.fetch = original;
    }
  },
};

export const RejectUnsignedAccount: Story = {
  play: async ({ canvasElement }) => {
    const c = within(canvasElement);
    const original = globalThis.fetch;
    globalThis.fetch = (async (input, init) => {
      const url = String(input);
      const method = init?.method ?? "GET";
      if (url.endsWith("/accounts/active") && method === "GET") {
        return new Response(
          JSON.stringify({
            accounts: [
              {
                is_active: true,
                account: {
                  id: activeId,
                  identifier: "alice@example.com",
                  display_name: "Alice",
                  org: null,
                },
              },
              {
                is_active: false,
                account: {
                  id: secondId,
                  identifier: "alice@work.example",
                  display_name: "Alice Work",
                  org: "00000000-0000-7000-8000-000000000020",
                },
              },
            ],
          }),
          { status: 200, headers: { "content-type": "application/json" } },
        );
      }
      if (url.endsWith("/accounts/active/switch") && method === "POST") {
        return new Response(
          JSON.stringify({
            code: "target_account_not_signed_in",
            summary: "The requested target account is not currently signed in.",
          }),
          { status: 403, headers: { "content-type": "application/json" } },
        );
      }
      return new Response(null, { status: 404 });
    }) as typeof fetch;

    try {
      const select = (await c.findByRole("combobox")) as HTMLSelectElement;
      await expect(select.value).toBe(activeId);
      await userEvent.selectOptions(select, secondId);
      const alert = await c.findByRole("alert");
      await expect(alert).toBeVisible();
      await waitFor(() => {
        expect(select.value).toBe(activeId);
      });
    } finally {
      globalThis.fetch = original;
    }
  },
};

export const PhoneWidth: Story = {
  parameters: {
    viewport: { defaultViewport: "mobile1" },
  },
  play: async ({ canvasElement }) => {
    const c = within(canvasElement);
    const original = globalThis.fetch;
    globalThis.fetch = (async () =>
      new Response(
        JSON.stringify({
          accounts: [
            {
              is_active: true,
              account: {
                id: activeId,
                identifier: "alice@example.com",
                display_name: "Alice",
                org: null,
              },
            },
          ],
        }),
        { status: 200, headers: { "content-type": "application/json" } },
      )) as typeof fetch;

    try {
      await expect(await c.findByRole("combobox")).toBeVisible();
      await expect(c.getByRole("button")).toBeVisible();
    } finally {
      globalThis.fetch = original;
    }
  },
};
