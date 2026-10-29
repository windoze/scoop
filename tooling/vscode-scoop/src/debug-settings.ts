import { spawn } from "node:child_process";
import { access, constants } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { createInterface } from "node:readline";

export interface DebugBuild {
  input?: string;
  tests?: boolean;
  filter?: string;
  profile?: "debug" | "release";
  args?: string[];
}

export async function toolCommand(
  name: string,
  configured: string,
): Promise<string> {
  if (configured) {
    return configured.startsWith("~/")
      ? path.join(os.homedir(), configured.slice(2))
      : configured;
  }
  if (process.platform === "win32") {
    name = name === "scoop" ? "sco.exe" : `${name}.exe`;
  }
  const installed = path.join(os.homedir(), ".scoop/bin", name);
  try {
    await access(installed, constants.X_OK);
    return installed;
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code !== "ENOENT") throw error;
    return name;
  }
}

export function buildArguments(build: DebugBuild, sysroot: string): string[] {
  return [
    build.tests ? "test" : "build",
    build.input ?? ".",
    ...(build.tests ? ["--no-run"] : []),
    ...(build.args ?? []),
    "--profile",
    build.profile ?? "debug",
    "--debug-info",
    "full",
    "--message-format",
    "json",
    ...(sysroot ? ["--sysroot", sysroot] : []),
  ];
}

export function testArguments(build: DebugBuild): string[] {
  return build.tests && build.filter !== undefined
    ? ["--scoop-test-filter", build.filter]
    : [];
}

export function formatterCommand(file: string): string {
  if (/[\r\n\0]/u.test(file)) throw new Error("Invalid formatter path");
  // This is an LLDB command argument, never a shell command or Python source.
  return `command script import "${file.replaceAll("\\", "\\\\").replaceAll('"', '\\"')}"`;
}

export function buildProgram(
  command: string,
  args: string[],
  cwd: string,
  output: (line: string) => void,
  signal?: AbortSignal,
): Promise<string> {
  return new Promise((resolve, reject) => {
    const executables: string[] = [];
    const child = spawn(command, args, {
      cwd,
      stdio: ["ignore", "pipe", "pipe"],
      signal,
    });
    createInterface({ input: child.stdout }).on("line", output);
    createInterface({ input: child.stderr }).on("line", (line) => {
      output(line);
      try {
        const record = JSON.parse(line);
        if (
          record.schema === 1 &&
          record.kind === "executable" &&
          typeof record.output === "string"
        ) {
          executables.push(record.output);
        }
      } catch {
        // Compiler/tool diagnostics can include ordinary text as well as JSON.
      }
    });
    child.on("error", reject);
    child.on("close", (code) => {
      if (code !== 0) {
        reject(
          new Error(
            `Scoop build failed (exit ${code}); see Scoop Debug output.`,
          ),
        );
      } else if (executables.length !== 1) {
        reject(
          new Error(
            "Select one executable Cone or source file in build.input.",
          ),
        );
      } else {
        resolve(executables[0]);
      }
    });
  });
}
