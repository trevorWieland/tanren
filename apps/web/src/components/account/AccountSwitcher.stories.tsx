import type { Meta, StoryObj } from "@storybook/nextjs-vite";
import { expect, userEvent, waitFor, within } from "storybook/test";

import {
  AccountRequestError,
  parseAccountId,
  type SignedInAccountView,
} from "@/app/lib/account-client";
import type { AccountId } from "@/app/lib/generated/account-contract";

import { AccountSwitcher } from "./AccountSwitcher";

const activeId = "00000000-0000-7000-8000-000000000010";
const secondId = "00000000-0000-7000-8000-000000000011";

function requireAccountId(value: string): AccountId {
  const parsed = parseAccountId(value);
  if (parsed === null) {
    throw new Error(`invalid story account id: ${value}`);
  }
  return parsed;
}

function makeAccounts(active: "first" | "second"): SignedInAccountView[] {
  const firstAccountId = requireAccountId(activeId);
  const secondAccountId = requireAccountId(secondId);
  return [
    {
      is_active: active === "first",
      account: {
        id: firstAccountId,
        display_name: "Alice",
        org: null,
      },
    },
    {
      is_active: active === "second",
      account: {
        id: secondAccountId,
        display_name: "Alice Work",
        org: null,
      },
    },
  ];
}

const defaultListAccounts = async () => ({
  accounts: makeAccounts("first"),
});

const defaultSwitchAccount = async ({
  target_account_id,
}: {
  target_account_id: AccountId;
}) => ({
  active_account_id:
    target_account_id === requireAccountId(secondId)
      ? requireAccountId(secondId)
      : requireAccountId(activeId),
  accounts:
    target_account_id === requireAccountId(secondId)
      ? makeAccounts("second")
      : makeAccounts("first"),
});

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
  args: {
    listAccounts: defaultListAccounts,
    switchAccount: defaultSwitchAccount,
  },
  play: async ({ canvasElement }) => {
    const c = within(canvasElement);
    const select = (await c.findByRole("combobox")) as HTMLSelectElement;
    await waitFor(() => {
      expect(select.value).toBe(activeId);
    });
    await expect(select).toBeVisible();

    await userEvent.selectOptions(select, secondId);
    await waitFor(() => {
      expect(select.value).toBe(secondId);
    });

    await userEvent.click(c.getByRole("button", { name: /refresh/i }));
    await waitFor(() => {
      expect(select.value).toBe(activeId);
    });
  },
};

export const RejectUnsignedAccount: Story = {
  args: {
    listAccounts: defaultListAccounts,
    switchAccount: async () => {
      throw new AccountRequestError({
        code: "target_account_not_signed_in",
        summary: "The requested target account is not currently signed in.",
      });
    },
  },
  play: async ({ canvasElement }) => {
    const c = within(canvasElement);
    const select = (await c.findByRole("combobox")) as HTMLSelectElement;
    await waitFor(() => {
      expect(select.value).toBe(activeId);
    });

    await userEvent.selectOptions(select, secondId);
    const alert = await c.findByRole("alert");
    await expect(alert).toBeVisible();
    await waitFor(() => {
      expect(select.value).toBe(activeId);
    });
  },
};

export const PhoneWidth: Story = {
  args: {
    listAccounts: async () => ({
      accounts: [
        {
          is_active: true,
          account: {
            id: requireAccountId(activeId),
            display_name: "Alice",
            org: null,
          },
        },
      ],
    }),
    switchAccount: defaultSwitchAccount,
  },
  parameters: {
    viewport: { defaultViewport: "mobile1" },
  },
  play: async ({ canvasElement }) => {
    const c = within(canvasElement);
    await expect(await c.findByRole("combobox")).toBeVisible();
    await expect(c.getByRole("button")).toBeVisible();
  },
};
