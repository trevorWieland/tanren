import { createHash } from "node:crypto";
import { lstat, readdir, readFile, readlink, realpath } from "node:fs/promises";
import { isAbsolute, join, relative, resolve } from "node:path";

const INSTALL_MANIFEST_PATH = ".tanren/install-manifest.toml";
const TANREN_METADATA_ROOT = ".tanren";
const STANDARDS_PROFILE_ROOT = "profiles/rust-cargo/";
const LEGACY_GENERATED_ASSET_PATHS = [
  ".codex/skills/retired-command.md",
] as const;
const GENERATED_COMMAND_ROOTS = [
  ".claude/commands/",
  ".codex/skills/",
  ".opencode/commands/",
] as const;

export interface RepositorySnapshot {
  readonly directories: readonly string[];
  readonly files: readonly [string, string][];
  readonly symlinks: readonly [string, string][];
}

interface SnapshotParts {
  readonly directories: string[];
  readonly files: [string, string][];
  readonly symlinks: [string, string][];
}

let catalogGeneratedPathCache: Promise<ReadonlySet<string>> | undefined;

export async function snapshotRepositoryForUpgradeAssertions(
  repositoryRoot: string,
  workspaceRoot: string,
  explicitScenarioPaths: ReadonlySet<string>,
): Promise<RepositorySnapshot> {
  const root = await realpath(repositoryRoot);
  const [catalogGeneratedPaths, manifestGeneratedPaths] = await Promise.all([
    loadCatalogGeneratedPaths(workspaceRoot),
    loadManifestGeneratedPaths(repositoryRoot),
  ]);

  const exactPaths = new Set<string>([
    INSTALL_MANIFEST_PATH,
    ...catalogGeneratedPaths,
    ...manifestGeneratedPaths,
    ...explicitScenarioPaths,
  ]);

  const parts: SnapshotParts = {
    directories: [],
    files: [],
    symlinks: [],
  };
  const visited = new Set<string>();
  for (const trackedPath of exactPaths) {
    await captureTrackedPath(root, trackedPath, parts, visited);
  }
  await captureTrackedPath(root, TANREN_METADATA_ROOT, parts, visited);

  parts.directories.sort((left, right) => left.localeCompare(right));
  parts.files.sort(compareTuple);
  parts.symlinks.sort(compareTuple);

  return {
    directories: parts.directories,
    files: parts.files,
    symlinks: parts.symlinks,
  };
}

export function sha256Hex(payload: string | Uint8Array): string {
  return createHash("sha256").update(payload).digest("hex");
}

function compareTuple(left: [string, string], right: [string, string]): number {
  const pathCompare = left[0].localeCompare(right[0]);
  if (pathCompare !== 0) {
    return pathCompare;
  }
  return left[1].localeCompare(right[1]);
}

async function captureTrackedPath(
  root: string,
  trackedPath: string,
  parts: SnapshotParts,
  visited: Set<string>,
): Promise<void> {
  const relativePath = normalizeRelativePath(trackedPath);
  if (!isValidRelativePath(relativePath) || visited.has(relativePath)) {
    return;
  }
  visited.add(relativePath);

  const absolutePath = resolve(root, relativePath);
  let stats;
  try {
    stats = await lstat(absolutePath);
  } catch (error) {
    const enoent =
      typeof error === "object" &&
      error !== null &&
      "code" in error &&
      (error as { code?: string }).code === "ENOENT";
    if (enoent) {
      return;
    }
    throw error;
  }

  if (stats.isDirectory()) {
    parts.directories.push(relativePath);
    const entries = await readdir(absolutePath, { withFileTypes: true });
    entries.sort((left, right) => left.name.localeCompare(right.name));
    for (const entry of entries) {
      const childPath = normalizeRelativePath(`${relativePath}/${entry.name}`);
      await captureTrackedPath(root, childPath, parts, visited);
    }
    return;
  }

  if (stats.isSymbolicLink()) {
    const target = await readlink(absolutePath);
    parts.symlinks.push([relativePath, normalizeRelativePath(target)]);
    return;
  }

  if (!stats.isFile()) {
    return;
  }

  const bytes = await readFile(absolutePath);
  parts.files.push([relativePath, sha256Hex(bytes)]);
}

async function loadCatalogGeneratedPaths(
  workspaceRoot: string,
): Promise<ReadonlySet<string>> {
  if (catalogGeneratedPathCache) {
    return catalogGeneratedPathCache;
  }
  catalogGeneratedPathCache = buildCatalogGeneratedPaths(workspaceRoot);
  return catalogGeneratedPathCache;
}

async function buildCatalogGeneratedPaths(
  workspaceRoot: string,
): Promise<ReadonlySet<string>> {
  const generatedPaths = new Set<string>();
  for (const path of LEGACY_GENERATED_ASSET_PATHS) {
    generatedPaths.add(path);
  }

  const commandNames = await listCommandNames(workspaceRoot);
  for (const commandName of commandNames) {
    for (const destinationRoot of GENERATED_COMMAND_ROOTS) {
      generatedPaths.add(`${destinationRoot}${commandName}.md`);
    }
  }

  const standardsRootAbsolute = resolve(workspaceRoot, STANDARDS_PROFILE_ROOT);
  const standardsFiles = await listFilesRecursive(standardsRootAbsolute);
  for (const absolutePath of standardsFiles) {
    const relativePath = normalizeRelativePath(
      relative(workspaceRoot, absolutePath),
    );
    if (isValidRelativePath(relativePath)) {
      generatedPaths.add(relativePath);
    }
  }

  return generatedPaths;
}

async function listCommandNames(
  workspaceRoot: string,
): Promise<readonly string[]> {
  const projectCommandsRoot = resolve(workspaceRoot, "commands/project");
  const entries = await readdir(projectCommandsRoot, { withFileTypes: true });
  entries.sort((left, right) => left.name.localeCompare(right.name));
  const commandNames: string[] = [];
  for (const entry of entries) {
    if (!entry.isFile() || !entry.name.endsWith(".md")) {
      continue;
    }
    commandNames.push(entry.name.slice(0, -3));
  }
  return commandNames;
}

async function listFilesRecursive(
  directory: string,
): Promise<readonly string[]> {
  const entries = await readdir(directory, { withFileTypes: true });
  entries.sort((left, right) => left.name.localeCompare(right.name));
  const files: string[] = [];
  for (const entry of entries) {
    const absolutePath = join(directory, entry.name);
    if (entry.isDirectory()) {
      const nested = await listFilesRecursive(absolutePath);
      for (const nestedPath of nested) {
        files.push(nestedPath);
      }
      continue;
    }
    if (entry.isFile()) {
      files.push(absolutePath);
    }
  }
  return files;
}

async function loadManifestGeneratedPaths(
  repositoryRoot: string,
): Promise<ReadonlySet<string>> {
  const manifestPath = resolve(repositoryRoot, INSTALL_MANIFEST_PATH);
  let manifestRaw: string;
  try {
    manifestRaw = await readFile(manifestPath, "utf-8");
  } catch (error) {
    const enoent =
      typeof error === "object" &&
      error !== null &&
      "code" in error &&
      (error as { code?: string }).code === "ENOENT";
    if (enoent) {
      return new Set<string>();
    }
    throw error;
  }

  const paths = new Set<string>();
  const pathLines = manifestRaw.matchAll(/^path = "([^"]+)"$/gm);
  for (const pathLine of pathLines) {
    const rawPath = pathLine[1];
    if (!rawPath) {
      continue;
    }
    const normalized = normalizeRelativePath(rawPath);
    if (isValidRelativePath(normalized)) {
      paths.add(normalized);
    }
  }
  return paths;
}

function normalizeRelativePath(value: string): string {
  const trimmed = value.trim().replaceAll("\\", "/");
  if (trimmed === ".") {
    return "";
  }
  return trimmed.replace(/^\.\/+/, "").replace(/\/+$/, "");
}

function isValidRelativePath(path: string): boolean {
  if (path.length === 0 || isAbsolute(path) || path.includes("\\")) {
    return false;
  }
  const segments = path.split("/");
  for (const segment of segments) {
    if (segment.length === 0 || segment === "." || segment === "..") {
      return false;
    }
  }
  return true;
}
