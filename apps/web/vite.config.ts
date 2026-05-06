import { resolve } from "node:path";

import { defineConfig } from "vite";

export default defineConfig({
  resolve: {
    alias: {
      "@": resolve(__dirname, "./src"),
    },
  },
  define: {
    "process.env.NEXT_PUBLIC_API_URL": JSON.stringify(
      process.env["NEXT_PUBLIC_API_URL"] ?? "http://localhost:8080",
    ),
  },
});
