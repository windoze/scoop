import { access, constants } from "node:fs/promises";
import os from "node:os";
import path from "node:path";

const optionNames = [
  "target",
  "sysroot",
  "purpose",
  "artifactSearchRoots",
  "configurations",
  "activeConfiguration",
] as const;

export function analysisOptions(settings: Record<string, unknown>) {
  // The server rejects unknown fields; client-only settings stay in VS Code.
  return Object.fromEntries(
    optionNames
      .filter((key) => settings[key] !== undefined && settings[key] !== "")
      .map((key) => [key, settings[key]]),
  );
}

export async function serverCommand(configured: string): Promise<string> {
  if (configured) {
    return configured.startsWith("~/") || configured.startsWith("~\\")
      ? path.join(os.homedir(), configured.slice(2))
      : configured;
  }
  const installed = path.join(
    os.homedir(),
    ".scoop/bin",
    process.platform === "win32" ? "scoop-lsp.exe" : "scoop-lsp",
  );
  try {
    await access(installed, constants.X_OK);
    return installed;
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code !== "ENOENT") throw error;
    return "scoop-lsp";
  }
}
