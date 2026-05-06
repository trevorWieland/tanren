import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig, loadEnv } from "vite";
import react from "@vitejs/plugin-react";

const __dirname = dirname(fileURLToPath(import.meta.url));

export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, process.cwd(), ["VITE_", "NEXT_PUBLIC_"]);
  return {
    plugins: [react()],
    resolve: {
      alias: {
        "@": resolve(__dirname, "./src"),
      },
    },
    define: {
      "process.env.NEXT_PUBLIC_API_URL": JSON.stringify(
        env["NEXT_PUBLIC_API_URL"] ?? "",
      ),
    },
    envPrefix: ["VITE_", "NEXT_PUBLIC_"],
  };
});
