import type { Meta, StoryObj } from "@storybook/nextjs-vite";
import { expect, userEvent, waitFor, within } from "storybook/test";

import { InvitationList } from "./InvitationList";
import type { OrgInvitationView } from "@/app/lib/account-client";

const orgId = "00000000-0000-7000-8000-0000000000a0";

const now = new Date().toISOString();
const futureExpiry = "2030-01-01T00:00:00Z";

const pendingInvitation: OrgInvitationView = {
  token: "story-pending-token-padpad",
  org_id: orgId,
  recipient_identifier: "alice@example.com",
  permissions: ["admin", "member"],
  status: "pending",
  creator: "00000000-0000-7000-8000-000000000001",
  created_at: now,
  expires_at: futureExpiry,
  revoked_at: null,
};

const revokedInvitation: OrgInvitationView = {
  token: "story-revoked-token-padpad",
  org_id: orgId,
  recipient_identifier: "bob@example.com",
  permissions: ["member"],
  status: "revoked",
  creator: "00000000-0000-7000-8000-000000000001",
  created_at: now,
  expires_at: futureExpiry,
  revoked_at: now,
};

const meta = {
  title: "Account/InvitationList",
  component: InvitationList,
  parameters: {
    a11y: { test: "error" },
    layout: "centered",
  },
  args: {
    orgId,
    initialInvitations: [],
  },
} satisfies Meta<typeof InvitationList>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
  args: {
    initialInvitations: [pendingInvitation],
  },
  play: async ({ canvasElement }) => {
    const c = within(canvasElement);
    await expect(c.getByText("alice@example.com")).toBeVisible();
    await expect(c.getByText("Pending")).toBeVisible();
    await expect(c.getByText("admin")).toBeVisible();
    await expect(c.getByText("member")).toBeVisible();
    await expect(c.getByRole("button", { name: /revoke/i })).toBeEnabled();
  },
};

export const Empty: Story = {
  args: {
    initialInvitations: [],
  },
  play: async ({ canvasElement }) => {
    const c = within(canvasElement);
    await expect(c.getByText(/no invitations yet/i)).toBeVisible();
  },
};

export const WithError: Story = {
  args: {
    initialInvitations: [pendingInvitation],
  },
  play: async ({ canvasElement }) => {
    const c = within(canvasElement);
    const original = globalThis.fetch;
    globalThis.fetch = (async () =>
      new Response(
        JSON.stringify({
          code: "permission_denied",
          summary: "Not allowed",
        }),
        { status: 403, headers: { "content-type": "application/json" } },
      )) as typeof fetch;
    try {
      const btn = c.getByRole("button", { name: /revoke/i });
      await userEvent.click(btn);
      await waitFor(() => {
        expect(c.getByRole("alert")).toBeVisible();
      });
    } finally {
      globalThis.fetch = original;
    }
  },
};

export const WithRevoked: Story = {
  args: {
    initialInvitations: [pendingInvitation, revokedInvitation],
  },
  play: async ({ canvasElement }) => {
    const c = within(canvasElement);
    const items = c.getAllByRole("listitem");
    await expect(items).toHaveLength(2);
    await expect(c.getByText("Pending")).toBeVisible();
    await expect(c.getByText("Revoked")).toBeVisible();
    await expect(c.getByText("bob@example.com")).toBeVisible();
  },
};
