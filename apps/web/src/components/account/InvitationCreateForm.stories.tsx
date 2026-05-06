import type { Meta, StoryObj } from "@storybook/nextjs-vite";
import { expect, fn, userEvent, waitFor, within } from "storybook/test";

import { InvitationCreateForm } from "./InvitationCreateForm";

const meta = {
  title: "Account/InvitationCreateForm",
  component: InvitationCreateForm,
  parameters: {
    a11y: { test: "error" },
    layout: "centered",
  },
  args: {
    orgId: "00000000-0000-7000-8000-0000000000a0",
    onSuccess: fn(),
  },
} satisfies Meta<typeof InvitationCreateForm>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
  play: async ({ canvasElement }) => {
    const c = within(canvasElement);
    await expect(c.getByLabelText(/recipient email/i)).toBeVisible();
    await expect(c.getByLabelText(/permissions/i)).toBeVisible();
    await expect(c.getByLabelText(/expires at/i)).toBeVisible();
    await expect(
      c.getByRole("button", { name: /send invitation/i }),
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
      await userEvent.type(
        c.getByLabelText(/recipient email/i),
        "user@example.com",
      );
      await userEvent.type(c.getByLabelText(/permissions/i), "admin");
      await userEvent.type(c.getByLabelText(/expires at/i), "2030-01-01T00:00");
      await userEvent.click(
        c.getByRole("button", { name: /send invitation/i }),
      );
      await waitFor(() => {
        expect(c.getByRole("button", { name: /sending/i })).toBeDisabled();
      });
    } finally {
      globalThis.fetch = original;
    }
  },
};

export const WithError: Story = {
  play: async ({ canvasElement }) => {
    const c = within(canvasElement);
    await userEvent.click(c.getByRole("button", { name: /send invitation/i }));
    const alert = await c.findByRole("alert");
    await expect(alert).toBeVisible();
    await expect(c.getByLabelText(/recipient email/i)).toHaveAttribute(
      "aria-invalid",
      "true",
    );
  },
};

export const Success: Story = {
  play: async ({ canvasElement, args }) => {
    const c = within(canvasElement);
    const original = globalThis.fetch;
    const orgId = "00000000-0000-7000-8000-0000000000a0";
    globalThis.fetch = (async () =>
      new Response(
        JSON.stringify({
          invitation: {
            token: "story-token-padpad",
            org_id: orgId,
            recipient_identifier: "user@example.com",
            permissions: ["admin"],
            status: "pending",
            creator: "00000000-0000-7000-8000-000000000001",
            created_at: new Date().toISOString(),
            expires_at: "2030-01-01T00:00:00Z",
            revoked_at: null,
          },
        }),
        { status: 201, headers: { "content-type": "application/json" } },
      )) as typeof fetch;
    try {
      await userEvent.type(
        c.getByLabelText(/recipient email/i),
        "user@example.com",
      );
      await userEvent.type(c.getByLabelText(/permissions/i), "admin");
      await userEvent.type(c.getByLabelText(/expires at/i), "2030-01-01T00:00");
      await userEvent.click(
        c.getByRole("button", { name: /send invitation/i }),
      );
      await waitFor(() => {
        expect(args.onSuccess).toHaveBeenCalledTimes(1);
      });
    } finally {
      globalThis.fetch = original;
    }
  },
};
