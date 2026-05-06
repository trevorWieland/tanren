import { setProjectAnnotations } from "@storybook/react-vite";
import { beforeAll } from "vitest";

import preview from "./.storybook/preview";

const project = setProjectAnnotations([preview]);

beforeAll(project.beforeAll);
