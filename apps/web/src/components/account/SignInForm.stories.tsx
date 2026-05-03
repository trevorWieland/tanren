import type { Meta, StoryObj } from "@storybook/nextjs-vite";
import { expect, fn, userEvent, waitFor, within } from "storybook/test";

import { SignInForm } from "./SignInForm";

// Visual contract for `SignInForm`. Behavior proof for `@web` sign-in
// lives in `tests/bdd/features/B-0043-create-account.feature` (driven by
// playwright-bdd in CI; in-process fallback for fast Rust BDD).
const meta = {
  title: "Account/SignInForm",
  component: SignInForm,
  parameters: {
    a11y: { test: "error" },
    layout: "centered",
  },
  args: {
    onSuccess: fn(),
  },
} satisfies Meta<typeof SignInForm>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
  play: async ({ canvasElement }) => {
    const c = within(canvasElement);
    await expect(c.getByLabelText(/email/i)).toBeVisible();
    await expect(c.getByLabelText(/password/i)).toBeVisible();
    await expect(c.getByRole("button", { name: /^sign in$/i })).toBeEnabled();
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
      await userEvent.click(c.getByRole("button", { name: /^sign in$/i }));
      await waitFor(() => {
        expect(c.getByRole("button", { name: /signing in/i })).toBeDisabled();
      });
    } finally {
      globalThis.fetch = original;
    }
  },
};

export const WithError: Story = {
  play: async ({ canvasElement }) => {
    const c = within(canvasElement);
    await userEvent.click(c.getByRole("button", { name: /^sign in$/i }));
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
    const accountId = "00000000-0000-7000-8000-000000000002";
    globalThis.fetch = (async () =>
      new Response(
        JSON.stringify({
          account: {
            id: accountId,
            identifier: "user@example.com",
            display_name: "User",
            org: null,
          },
          session: {
            account_id: accountId,
            expires_at: new Date(Date.now() + 60_000).toISOString(),
          },
        }),
        { status: 200, headers: { "content-type": "application/json" } },
      )) as typeof fetch;
    try {
      await userEvent.type(c.getByLabelText(/email/i), "user@example.com");
      await userEvent.type(c.getByLabelText(/password/i), "p4ssw0rd!");
      await userEvent.click(c.getByRole("button", { name: /^sign in$/i }));
      await waitFor(() => {
        expect(args.onSuccess).toHaveBeenCalledTimes(1);
      });
    } finally {
      globalThis.fetch = original;
    }
  },
};
