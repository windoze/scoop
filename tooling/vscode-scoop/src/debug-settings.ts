import { spawn } from "node:child_process";
import { access, constants } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { createInterface } from "node:readline";
import type { RunnableTarget } from "./runnables.ts";

export interface TestCaseSelection {
  path: string;
  container: string;
  name: string;
}

export interface TestDirectory {
  containers: {
    name: string;
    cases: { name: string; location: { path: string } }[];
  }[];
}

export interface BuiltProgram {
  program: string;
  testDirectory?: TestDirectory;
}

export interface DebugBuild {
  input?: string;
  tests?: boolean;
  filter?: string;
  testCase?: TestCaseSelection;
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

export function testArguments(
  build: DebugBuild,
  result?: BuiltProgram,
): string[] {
  if (build.tests && build.testCase) {
    if (!result?.testDirectory)
      throw new Error("Scoop build did not return a test directory.");
    const selected = build.testCase;
    let index = 0;
    const matches: number[] = [];
    for (const container of result.testDirectory.containers) {
      for (const test of container.cases) {
        if (
          container.name === selected.container &&
          test.name === selected.name &&
          test.location.path === selected.path
        )
          matches.push(index);
        index++;
      }
    }
    if (matches.length !== 1)
      throw new Error(
        `Cannot find one test ${selected.container}::${selected.name} in ${selected.path}. Refresh the CodeLens and try again.`,
      );
    return ["--scoop-test-cases", String(matches[0])];
  }
  return build.tests && build.filter !== undefined
    ? ["--scoop-test-filter", build.filter]
    : [];
}

export async function sourceBuildInput(file: string): Promise<{
  input: string;
  cwd: string;
  manifest: boolean;
}> {
  let directory = path.dirname(file);
  while (true) {
    try {
      await access(path.join(directory, "Cone.toml"));
      return { input: directory, cwd: directory, manifest: true };
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code !== "ENOENT") throw error;
    }
    const parent = path.dirname(directory);
    if (directory === parent)
      return { input: file, cwd: path.dirname(file), manifest: false };
    directory = parent;
  }
}

export async function sourceLaunchConfiguration(
  file: string,
  target?: RunnableTarget,
  noDebug = false,
) {
  const source = await sourceBuildInput(file);
  if (target?.kind === "test" && !source.manifest)
    throw new Error(
      "Scoop tests require a Cone.toml beside the source or in a parent directory.",
    );
  const build: DebugBuild = { input: source.input };
  if (target?.kind === "test") {
    build.tests = true;
    build.testCase = {
      path: path.relative(source.cwd, file).split(path.sep).join("/"),
      container: target.container,
      name: target.name,
    };
  }
  return {
    type: "scoop",
    request: "launch",
    name: `Scoop: ${noDebug ? "Run" : "Debug"} ${target?.kind === "test" ? `${target.container}::${target.name}` : path.basename(file)}`,
    cwd: source.cwd,
    build,
    console: "integratedTerminal",
    noDebug,
  };
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
): Promise<BuiltProgram> {
  return new Promise((resolve, reject) => {
    const executables: string[] = [];
    let testDirectory: TestDirectory | undefined;
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
          testDirectory = record.test_directory;
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
        resolve({ program: executables[0], testDirectory });
      }
    });
  });
}
