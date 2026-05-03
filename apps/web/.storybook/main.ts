import type { StorybookConfig } from "@storybook/nextjs-vite";

// Storybook 9 + @storybook/nextjs-vite is the modern Vite-based framework
// (NOT @storybook/nextjs which is webpack-based and incompatible with the
// Vitest addon). See profiles/react-ts-pnpm/testing/component-testing-via-storybook.md.
const config: StorybookConfig = {
  framework: "@storybook/nextjs-vite",
  stories: ["../src/**/*.stories.@(ts|tsx)"],
  addons: ["@storybook/addon-vitest", "@storybook/addon-a11y"],
  typescript: { reactDocgen: false },
};

export default config;
