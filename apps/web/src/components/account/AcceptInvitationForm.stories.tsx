import type { Meta, StoryObj } from "@storybook/nextjs-vite";
import { expect, fn, userEvent, waitFor, within } from "storybook/test";

import { AcceptInvitationForm } from "./AcceptInvitationForm";

// Visual contract for `AcceptInvitationForm`. Behavior proof for the
// `@web` acceptance flow lives in
// `tests/bdd/features/B-0043-create-account.feature`.
const meta = {
  title: "Account/AcceptInvitationForm",
  component: AcceptInvitationForm,
  parameters: {
    a11y: { test: "error" },
    layout: "centered",
  },
  args: {
    token: "story-fixture-token-padpad",
    onSuccess: fn(),
  },
} satisfies Meta<typeof AcceptInvitationForm>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
  play: async ({ canvasElement }) => {
    const c = within(canvasElement);
    await expect(c.getByLabelText(/email/i)).toBeVisible();
    await expect(c.getByLabelText(/password/i)).toBeVisible();
    await expect(c.getByLabelText(/display name/i)).toBeVisible();
    await expect(
      c.getByRole("button", { name: /accept and join/i }),
    ).toBeEnabled();
  },
};

export const Submitting: Story = {
  play: async ({ canvasElement }) => {
    const c = within(canvasElement);
    const original = globalThis.fetch;
    globalThis.fetch = (() =>
      new Promise(() => {
        /* never resolves */
      })) as typeof fetch;
    try {
      await userEvent.type(c.getByLabelText(/email/i), "user@example.com");
      await userEvent.type(c.getByLabelText(/password/i), "p4ssw0rd!");
      await userEvent.type(c.getByLabelText(/display name/i), "User");
      await userEvent.click(
        c.getByRole("button", { name: /accept and join/i }),
      );
      await waitFor(() => {
        expect(c.getByRole("button", { name: /joining/i })).toBeDisabled();
      });
    } finally {
      globalThis.fetch = original;
    }
  },
};

export const WithError: Story = {
  play: async ({ canvasElement }) => {
    const c = within(canvasElement);
    await userEvent.click(c.getByRole("button", { name: /accept and join/i }));
    const alert = await c.findByRole("alert");
    await expect(alert).toBeVisible();
    await expect(c.getByLabelText(/email/i)).toHaveAttribute(
      "aria-invalid",
      "true",
    );
  },
};

export const Success: Story = {
  play: async ({ canvasElement, args }) => {
    const c = within(canvasElement);
    const original = globalThis.fetch;
    const accountId = "00000000-0000-7000-8000-000000000003";
    const orgId = "00000000-0000-7000-8000-0000000000a0";
    globalThis.fetch = (async () =>
      new Response(
        JSON.stringify({
          account: {
            id: accountId,
            identifier: "user@example.com",
            display_name: "User",
            org: orgId,
          },
          session: {
            account_id: accountId,
            expires_at: new Date(Date.now() + 60_000).toISOString(),
          },
          joined_org: orgId,
        }),
        { status: 200, headers: { "content-type": "application/json" } },
      )) as typeof fetch;
    try {
      await userEvent.type(c.getByLabelText(/email/i), "user@example.com");
      await userEvent.type(c.getByLabelText(/password/i), "p4ssw0rd!");
      await userEvent.type(c.getByLabelText(/display name/i), "User");
      await userEvent.click(
        c.getByRole("button", { name: /accept and join/i }),
      );
      await waitFor(() => {
        expect(args.onSuccess).toHaveBeenCalledTimes(1);
      });
    } finally {
      globalThis.fetch = original;
    }
  },
};
