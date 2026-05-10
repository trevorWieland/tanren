import { spawn } from "node:child_process";
import { mkdir, rm } from "node:fs/promises";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const appRoot = path.resolve(scriptDir, "..");
const projectPath = "./src/i18n/project.inlang";
const outputPath = path.join(appRoot, "src/i18n/paraglide");
const isWatch = process.argv.includes("--watch");

async function prepareOutputDir() {
  await rm(outputPath, { recursive: true, force: true });
  await mkdir(outputPath, { recursive: true });
}

function runParaglide() {
  const args = [
    "exec",
    "paraglide-js",
    "compile",
    "--project",
    projectPath,
    "--outdir",
    "./src/i18n/paraglide",
    "--emit-ts-declarations",
  ];

  if (isWatch) {
    args.push("--watch");
  }

  const child = spawn("pnpm", args, {
    cwd: appRoot,
    stdio: "inherit",
    env: process.env,
  });

  child.on("exit", (code, signal) => {
    if (signal) {
      process.kill(process.pid, signal);
      return;
    }
    process.exit(code ?? 1);
  });

  child.on("error", (error) => {
    console.error(error);
    process.exit(1);
  });
}

if (!isWatch) {
  await prepareOutputDir();
}

runParaglide();
