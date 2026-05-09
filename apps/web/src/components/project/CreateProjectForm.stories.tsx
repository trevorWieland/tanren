import type { Meta, StoryObj } from "@storybook/nextjs-vite";
import { expect, fn, userEvent, waitFor, within } from "storybook/test";

import { CreateProjectForm } from "./CreateProjectForm";

const meta = {
  title: "Project/CreateProjectForm",
  component: CreateProjectForm,
  parameters: {
    a11y: { test: "error" },
    layout: "centered",
  },
  args: {
    onSuccess: fn(),
  },
} satisfies Meta<typeof CreateProjectForm>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
  play: async ({ canvasElement }) => {
    const c = within(canvasElement);
    await expect(c.getByLabelText(/repository/i)).toBeVisible();
    await expect(c.getByLabelText(/designated host/i)).toBeVisible();
    await expect(c.getByLabelText(/select as active project/i)).toBeChecked();
    await expect(
      c.getByRole("button", { name: /create project/i }),
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
      await userEvent.type(c.getByLabelText(/repository/i), "acme/new-project");
      await userEvent.type(
        c.getByLabelText(/designated host/i),
        "fixture.example",
      );
      await userEvent.click(c.getByRole("button", { name: /create project/i }));
      await waitFor(() => {
        expect(
          c.getByRole("button", { name: /creating project/i }),
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
    await userEvent.click(c.getByRole("button", { name: /create project/i }));
    const alert = await c.findByRole("alert");
    await expect(alert).toBeVisible();
    await expect(c.getByLabelText(/repository/i)).toHaveAttribute(
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
          code: "provider_failure",
          summary: "provider unavailable",
        }),
        { status: 502, headers: { "content-type": "application/json" } },
      )) as typeof fetch;
    try {
      await userEvent.type(c.getByLabelText(/repository/i), "acme/new-project");
      await userEvent.type(
        c.getByLabelText(/designated host/i),
        "fixture.example",
      );
      await userEvent.click(c.getByRole("button", { name: /create project/i }));
      const alert = await c.findByRole("alert");
      await expect(alert).toBeVisible();
      await expect(alert).toHaveTextContent(/provider could not complete/i);
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
            id: "00000000-0000-7000-8000-0000000000e2",
            owning_account_id: "00000000-0000-7000-8000-0000000000b2",
            repository: { repository: "acme/new-project" },
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
      await userEvent.type(c.getByLabelText(/repository/i), "acme/new-project");
      await userEvent.type(
        c.getByLabelText(/designated host/i),
        "fixture.example",
      );
      await userEvent.click(c.getByRole("button", { name: /create project/i }));
      await waitFor(() => {
        expect(args.onSuccess).toHaveBeenCalledTimes(1);
      });
    } finally {
      globalThis.fetch = original;
    }
  },
};
