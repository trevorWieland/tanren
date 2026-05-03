import type { Preview } from "@storybook/nextjs-vite";

import "../src/app/globals.css";

const preview: Preview = {
  parameters: {
    a11y: {
      // axe-core runs after each story's play function. `error` severity
      // means accessibility violations fail the story (and the
      // component-test gate via the addon-vitest integration).
      test: "error",
    },
    layout: "centered",
  },
};

export default preview;
