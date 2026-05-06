import type { Meta, StoryObj } from "@storybook/nextjs-vite";
import { expect, fn, userEvent, waitFor, within } from "storybook/test";

import { UninstallPanel } from "./UninstallPanel";

const previewResponse = {
  preview: {
    to_remove: [
      ".tanren/install-manifest.json",
      ".tanren/standards/quality.md",
      ".tanren/standards/security.md",
    ],
    preserved: [
      {
        path: "specs/my-feature.md",
        reason: "UserOwned" as const,
      },
      {
        path: ".tanren/standards/custom-checks.md",
        reason: "ModifiedSinceInstall" as const,
      },
    ],
    manifest_path: ".tanren/install-manifest.json",
  },
  hosted_data_unchanged: true,
};

const applyResponse = {
  result: {
    removed: [".tanren/standards/quality.md", ".tanren/standards/security.md"],
    preserved: [
      {
        path: "specs/my-feature.md",
        reason: "UserOwned" as const,
      },
      {
        path: ".tanren/standards/custom-checks.md",
        reason: "ModifiedSinceInstall" as const,
      },
    ],
    manifest_removed: true,
  },
  hosted_data_unchanged: true,
};

const meta = {
  title: "Project/UninstallPanel",
  component: UninstallPanel,
  parameters: {
    a11y: { test: "error" },
    layout: "centered",
  },
  args: {
    onApplied: fn(),
  },
} satisfies Meta<typeof UninstallPanel>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
  play: async ({ canvasElement }) => {
    const c = within(canvasElement);
    await expect(c.getByLabelText(/repository path/i)).toBeVisible();
    await expect(
      c.getByRole("button", { name: /preview removals/i }),
    ).toBeEnabled();
  },
};

export const PreviewLoaded: Story = {
  play: async ({ canvasElement }) => {
    const c = within(canvasElement);
    const original = globalThis.fetch;
    globalThis.fetch = (async () =>
      new Response(JSON.stringify(previewResponse), {
        status: 200,
        headers: { "content-type": "application/json" },
      })) as typeof fetch;
    try {
      await userEvent.type(
        c.getByLabelText(/repository path/i),
        "/tmp/my-repo",
      );
      await userEvent.click(
        c.getByRole("button", { name: /preview removals/i }),
      );
      await waitFor(() => {
        expect(c.getByText(/files to remove/i)).toBeVisible();
      });
      await expect(c.getByText(/files preserved/i)).toBeVisible();
      await expect(c.getByText(/hosted account/i)).toBeVisible();
      await expect(
        c.getByRole("button", { name: /remove selected assets/i }),
      ).toBeDisabled();
    } finally {
      globalThis.fetch = original;
    }
  },
};

export const ApplySuccess: Story = {
  play: async ({ canvasElement, args }) => {
    const c = within(canvasElement);
    const original = globalThis.fetch;
    let callCount = 0;
    globalThis.fetch = (async () => {
      callCount += 1;
      if (callCount === 1) {
        return new Response(JSON.stringify(previewResponse), {
          status: 200,
          headers: { "content-type": "application/json" },
        });
      }
      return new Response(JSON.stringify(applyResponse), {
        status: 200,
        headers: { "content-type": "application/json" },
      });
    }) as typeof fetch;
    try {
      await userEvent.type(
        c.getByLabelText(/repository path/i),
        "/tmp/my-repo",
      );
      await userEvent.click(
        c.getByRole("button", { name: /preview removals/i }),
      );
      await waitFor(() => {
        expect(c.getByText(/files to remove/i)).toBeVisible();
      });
      await userEvent.click(c.getByRole("checkbox"));
      await userEvent.click(
        c.getByRole("button", { name: /remove selected assets/i }),
      );
      await waitFor(() => {
        expect(args.onApplied).toHaveBeenCalledTimes(1);
      });
      await expect(c.getByText(/uninstall complete/i)).toBeVisible();
    } finally {
      globalThis.fetch = original;
    }
  },
};

export const WithError: Story = {
  play: async ({ canvasElement }) => {
    const c = within(canvasElement);
    const original = globalThis.fetch;
    globalThis.fetch = (async () =>
      new Response(
        JSON.stringify({
          code: "manifest_not_found",
          summary: "No manifest found",
        }),
        { status: 404, headers: { "content-type": "application/json" } },
      )) as typeof fetch;
    try {
      await userEvent.type(
        c.getByLabelText(/repository path/i),
        "/tmp/my-repo",
      );
      await userEvent.click(
        c.getByRole("button", { name: /preview removals/i }),
      );
      const alert = await c.findByRole("alert");
      await expect(alert).toBeVisible();
    } finally {
      globalThis.fetch = original;
    }
  },
};
