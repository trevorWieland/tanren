import type { Preview } from "@storybook/react-vite";

import "../src/app/globals.css";

const preview: Preview = {
  parameters: {
    a11y: {
      test: "error",
    },
    layout: "centered",
  },
};

export default preview;
