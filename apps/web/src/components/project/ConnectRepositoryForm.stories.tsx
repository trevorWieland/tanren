import type { Meta, StoryObj } from "@storybook/nextjs-vite";
import { expect, fn, userEvent, waitFor, within } from "storybook/test";

import { ConnectRepositoryForm } from "./ConnectRepositoryForm";

const meta = {
  title: "Project/ConnectRepositoryForm",
  component: ConnectRepositoryForm,
  parameters: {
    a11y: { test: "error" },
    layout: "centered",
  },
  args: {
    onSuccess: fn(),
  },
} satisfies Meta<typeof ConnectRepositoryForm>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
  play: async ({ canvasElement }) => {
    const c = within(canvasElement);
    await expect(c.getByLabelText(/^repository$/i)).toBeVisible();
    await expect(c.getByLabelText(/select as active project/i)).toBeChecked();
    await expect(
      c.getByRole("button", { name: /connect repository/i }),
    ).toBeEnabled();
  },
};

export const Pending: Story = {
  play: async ({ canvasElement }) => {
    const c = within(canvasElement);
    const original = globalThis.fetch;
    globalThis.fetch = (() =>
      new Promise(() => {
        /* never resolves */
      })) as typeof fetch;
    try {
      await userEvent.type(c.getByLabelText(/^repository$/i), "acme/tanren");
      await userEvent.click(
        c.getByRole("button", { name: /connect repository/i }),
      );
      await waitFor(() => {
        expect(
          c.getByRole("button", { name: /connecting repository/i }),
        ).toBeDisabled();
      });
    } finally {
      globalThis.fetch = original;
    }
  },
};

export const ValidationFailure: Story = {
  play: async ({ canvasElement }) => {
    const c = within(canvasElement);
    await userEvent.click(
      c.getByRole("button", { name: /connect repository/i }),
    );
    const alert = await c.findByRole("alert");
    await expect(alert).toBeVisible();
    await expect(c.getByLabelText(/^repository$/i)).toHaveAttribute(
      "aria-invalid",
      "true",
    );
  },
};

export const RequestFailure: Story = {
  play: async ({ canvasElement }) => {
    const c = within(canvasElement);
    const original = globalThis.fetch;
    globalThis.fetch = (async () =>
      new Response(
        JSON.stringify({
          code: "duplicate_repository",
          summary: "duplicate",
        }),
        { status: 409, headers: { "content-type": "application/json" } },
      )) as typeof fetch;
    try {
      await userEvent.type(c.getByLabelText(/^repository$/i), "acme/tanren");
      await userEvent.click(
        c.getByRole("button", { name: /connect repository/i }),
      );
      const alert = await c.findByRole("alert");
      await expect(alert).toBeVisible();
      await expect(alert).toHaveTextContent(/already connected/i);
    } finally {
      globalThis.fetch = original;
    }
  },
};

export const Success: Story = {
  play: async ({ canvasElement, args }) => {
    const c = within(canvasElement);
    const original = globalThis.fetch;
    globalThis.fetch = (async () =>
      new Response(
        JSON.stringify({
          project: {
            id: "00000000-0000-7000-8000-0000000000e1",
            owning_account_id: "00000000-0000-7000-8000-0000000000b1",
            repository: {
              source_control_host: "source-control.local",
              repository: "acme/tanren",
            },
            selection: {
              is_active: true,
              selected_at: new Date(Date.now() - 5_000).toISOString(),
            },
            counts: { specs: 0, milestones: 0, initiatives: 0 },
            created_at: new Date().toISOString(),
          },
        }),
        { status: 201, headers: { "content-type": "application/json" } },
      )) as typeof fetch;
    try {
      await userEvent.type(c.getByLabelText(/^repository$/i), "acme/tanren");
      await userEvent.click(
        c.getByRole("button", { name: /connect repository/i }),
      );
      await waitFor(() => {
        expect(args.onSuccess).toHaveBeenCalledTimes(1);
      });
    } finally {
      globalThis.fetch = original;
    }
  },
};
