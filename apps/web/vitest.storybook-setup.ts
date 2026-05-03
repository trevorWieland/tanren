// Setup file consumed by the Vitest `storybook` project. Wires the
// Storybook addon-vitest setup hook so play functions and the a11y
// addon both run as real-browser component tests.
import { setProjectAnnotations } from "@storybook/nextjs-vite";
import { beforeAll } from "vitest";

import preview from "./.storybook/preview";

const project = setProjectAnnotations([preview]);

beforeAll(project.beforeAll);
