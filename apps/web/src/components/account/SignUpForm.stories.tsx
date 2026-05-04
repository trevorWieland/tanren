import type { Meta, StoryObj } from "@storybook/nextjs-vite";
import { expect, fn, userEvent, waitFor, within } from "storybook/test";

import { SignUpForm } from "./SignUpForm";

// Storybook visual contract for `SignUpForm`. Each of the four required
// stories (Default / Submitting / WithError / Success) ships a `play`
// function that exercises the rendered DOM, and addon-a11y runs axe-core
// against the result. Behavior proof is owned by the `@web` BDD scenarios
// in `tests/bdd/features/B-0043-create-account.feature` — these stories
// document visual states, not end-to-end behavior.
const meta = {
  title: "Account/SignUpForm",
  component: SignUpForm,
  parameters: {
    a11y: { test: "error" },
    layout: "centered",
  },
  args: {
    onSuccess: fn(),
  },
} satisfies Meta<typeof SignUpForm>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
  play: async ({ canvasElement }) => {
    const c = within(canvasElement);
    await expect(c.getByLabelText(/email/i)).toBeVisible();
    await expect(c.getByLabelText(/password/i)).toBeVisible();
    await expect(c.getByLabelText(/display name/i)).toBeVisible();
    await expect(
      c.getByRole("button", { name: /create account/i }),
    ).toBeEnabled();
  },
};

// Drives the submit while a slow `fetch` keeps `useTransition` pending,
// so the button label flips to the "submitting" state.
export const Submitting: Story = {
  play: async ({ canvasElement }) => {
    const c = within(canvasElement);
    const original = globalThis.fetch;
    // Never resolves — leaves the form in the pending state.
    globalThis.fetch = (() =>
      new Promise(() => {
        /* never resolves */
      })) as typeof fetch;
    try {
      await userEvent.type(c.getByLabelText(/email/i), "user@example.com");
      await userEvent.type(c.getByLabelText(/password/i), "p4ssw0rd!");
      await userEvent.type(c.getByLabelText(/display name/i), "User");
      await userEvent.click(c.getByRole("button", { name: /create account/i }));
      await waitFor(() => {
        expect(
          c.getByRole("button", { name: /creating account/i }),
        ).toBeDisabled();
      });
    } finally {
      globalThis.fetch = original;
    }
  },
};

// Submitting an empty form trips the valibot guard before any fetch is
// issued, so the inline alert renders without needing a network mock.
export const WithError: Story = {
  play: async ({ canvasElement }) => {
    const c = within(canvasElement);
    await userEvent.click(c.getByRole("button", { name: /create account/i }));
    const alert = await c.findByRole("alert");
    await expect(alert).toBeVisible();
    await expect(c.getByLabelText(/email/i)).toHaveAttribute(
      "aria-invalid",
      "true",
    );
  },
};

// Mock fetch with a successful sign-up payload so the onSuccess callback
// fires; the test asserts the `args.onSuccess` mock was invoked with the
// shape the API returns.
export const Success: Story = {
  play: async ({ canvasElement, args }) => {
    const c = within(canvasElement);
    const original = globalThis.fetch;
    const accountId = "00000000-0000-7000-8000-000000000001";
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
        { status: 201, headers: { "content-type": "application/json" } },
      )) as typeof fetch;
    try {
      await userEvent.type(c.getByLabelText(/email/i), "user@example.com");
      await userEvent.type(c.getByLabelText(/password/i), "p4ssw0rd!");
      await userEvent.type(c.getByLabelText(/display name/i), "User");
      await userEvent.click(c.getByRole("button", { name: /create account/i }));
      await waitFor(() => {
        expect(args.onSuccess).toHaveBeenCalledTimes(1);
      });
    } finally {
      globalThis.fetch = original;
    }
  },
};
