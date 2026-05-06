import type { Meta, StoryObj } from "@storybook/nextjs-vite";
import { expect, fn, userEvent, waitFor, within } from "storybook/test";

import { OrganizationCreateForm } from "./OrganizationCreateForm";

const meta = {
  title: "Organization/OrganizationCreateForm",
  component: OrganizationCreateForm,
  parameters: {
    a11y: { test: "error" },
    layout: "centered",
  },
  args: {
    onSuccess: fn(),
  },
} satisfies Meta<typeof OrganizationCreateForm>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
  play: async ({ canvasElement }) => {
    const c = within(canvasElement);
    await expect(c.getByLabelText(/organization name/i)).toBeVisible();
    await expect(
      c.getByRole("button", { name: /create organization/i }),
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
      await userEvent.type(c.getByLabelText(/organization name/i), "Test Org");
      await userEvent.click(
        c.getByRole("button", { name: /create organization/i }),
      );
      await waitFor(() => {
        expect(
          c.getByRole("button", { name: /creating organization/i }),
        ).toBeDisabled();
      });
    } finally {
      globalThis.fetch = original;
    }
  },
};

export const WithError: Story = {
  play: async ({ canvasElement }) => {
    const c = within(canvasElement);
    await userEvent.click(
      c.getByRole("button", { name: /create organization/i }),
    );
    const alert = await c.findByRole("alert");
    await expect(alert).toBeVisible();
    await expect(c.getByLabelText(/organization name/i)).toHaveAttribute(
      "aria-invalid",
      "true",
    );
  },
};

export const Success: Story = {
  play: async ({ canvasElement, args }) => {
    const c = within(canvasElement);
    const original = globalThis.fetch;
    const orgId = "00000000-0000-7000-8000-0000000000b0";
    const membershipId = "00000000-0000-7000-8000-0000000000b1";
    const accountId = "00000000-0000-7000-8000-000000000001";
    globalThis.fetch = (async () =>
      new Response(
        JSON.stringify({
          organization: {
            id: orgId,
            name: "Test Org",
            created_at: new Date(Date.now()).toISOString(),
          },
          membership: {
            id: membershipId,
            account_id: accountId,
            org_id: orgId,
            permissions: [
              "invite_members",
              "manage_access",
              "configure",
              "set_policy",
              "delete",
            ],
            created_at: new Date(Date.now()).toISOString(),
          },
        }),
        { status: 201, headers: { "content-type": "application/json" } },
      )) as typeof fetch;
    try {
      await userEvent.type(c.getByLabelText(/organization name/i), "Test Org");
      await userEvent.click(
        c.getByRole("button", { name: /create organization/i }),
      );
      await waitFor(() => {
        expect(args.onSuccess).toHaveBeenCalledTimes(1);
      });
    } finally {
      globalThis.fetch = original;
    }
  },
};
