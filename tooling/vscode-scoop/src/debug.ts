import { access } from "node:fs/promises";
import path from "node:path";
import * as vscode from "vscode";
import {
  buildArguments,
  buildProgram,
  type DebugBuild,
  formatterCommand,
  testArguments,
  toolCommand,
} from "./debug-settings.ts";

// biome-ignore lint/suspicious/noTemplateCurlyInString: VS Code resolves launch variables.
const workspaceVariable = "${workspaceFolder}";
// biome-ignore lint/suspicious/noTemplateCurlyInString: VS Code resolves launch variables.
const fileVariable = "${file}";

export function registerDebugging(context: vscode.ExtensionContext): void {
  const output = vscode.window.createOutputChannel("Scoop Debug");
  const initial = (input?: string): vscode.DebugConfiguration => ({
    type: "scoop",
    request: "launch",
    name: "Scoop: Build and debug",
    build: input ? { input } : true,
    console: "integratedTerminal",
  });
  const provider: vscode.DebugConfigurationProvider = {
    provideDebugConfigurations(folder) {
      return [
        initial(folder ? workspaceVariable : fileVariable),
        {
          ...initial(workspaceVariable),
          name: "Scoop: Debug tests",
          build: { input: workspaceVariable, tests: true },
        },
      ];
    },
    resolveDebugConfiguration(_folder, config) {
      if (!config.type) return initial();
      return config;
    },
    async resolveDebugConfigurationWithSubstitutedVariables(
      folder,
      config,
      cancellation,
    ) {
      if (!vscode.workspace.isTrusted) {
        void vscode.window.showErrorMessage(
          "Scoop debugging requires a trusted workspace.",
        );
        return undefined;
      }
      try {
        const settings = vscode.workspace.getConfiguration(
          "scoop",
          folder?.uri,
        );
        const active = vscode.window.activeTextEditor?.document;
        const cwd =
          config.cwd ??
          folder?.uri.fsPath ??
          (active?.uri.scheme === "file"
            ? path.dirname(active.uri.fsPath)
            : undefined);
        if (!cwd)
          throw new Error(
            "Open a workspace or save the source file before debugging.",
          );
        config.cwd = cwd;
        config.initCommands = [
          formatterCommand(
            context.asAbsolutePath("dist/debugger/scoop_lldb.py"),
          ),
          ...(config.initCommands ?? []),
        ];
        config.enableSyntheticChildDebugging ??= true;
        config.console ??= "integratedTerminal";
        if (config.request === "launch" && (config.build ?? !config.program)) {
          if (!(await vscode.workspace.saveAll(false))) return undefined;
          const build: DebugBuild =
            typeof config.build === "object" ? { ...config.build } : {};
          if (!build.input) {
            try {
              await access(path.join(cwd, "Cone.toml"));
              build.input = cwd;
            } catch (error) {
              if ((error as NodeJS.ErrnoException).code !== "ENOENT")
                throw error;
              if (
                active?.languageId !== "scoop" ||
                active.uri.scheme !== "file"
              ) {
                throw new Error(
                  "Set build.input to a Cone directory or saved .scoop file.",
                );
              }
              build.input = active.uri.fsPath;
            }
          }
          const command = await toolCommand(
            "scoop",
            settings.get("debug.compilerPath", ""),
          );
          output.clear();
          const args = buildArguments(build, settings.get("sysroot", ""));
          output.appendLine(
            [command, ...args].map((arg) => JSON.stringify(arg)).join(" "),
          );
          config.program = await vscode.window.withProgress(
            {
              location: vscode.ProgressLocation.Notification,
              title: "Building Scoop debug target",
              cancellable: true,
            },
            async (_progress, token) => {
              const abort = new AbortController();
              const listeners = [token, cancellation].map((current) =>
                current?.onCancellationRequested(() => abort.abort()),
              );
              if (
                token.isCancellationRequested ||
                cancellation?.isCancellationRequested
              )
                abort.abort();
              try {
                return await buildProgram(
                  command,
                  args,
                  cwd,
                  (line) => output.appendLine(line),
                  abort.signal,
                );
              } finally {
                for (const listener of listeners) listener?.dispose();
              }
            },
          );
          config.args = [...testArguments(build), ...(config.args ?? [])];
        }
        delete config.build;
        return cancellation?.isCancellationRequested ? undefined : config;
      } catch (error) {
        output.show(true);
        void vscode.window.showErrorMessage(
          error instanceof Error ? error.message : String(error),
        );
        return undefined;
      }
    },
  };
  context.subscriptions.push(
    output,
    vscode.debug.registerDebugConfigurationProvider("scoop", provider),
    vscode.debug.registerDebugAdapterDescriptorFactory("scoop", {
      async createDebugAdapterDescriptor(session) {
        const settings = vscode.workspace.getConfiguration(
          "scoop",
          session.workspaceFolder?.uri,
        );
        const command = await toolCommand(
          "scoop-lldb-dap",
          settings.get("debug.adapterPath", ""),
        );
        return new vscode.DebugAdapterExecutable(command, [
          "--repl-mode=variable",
        ]);
      },
    }),
    vscode.commands.registerCommand("scoop.debugFile", async () => {
      const document = vscode.window.activeTextEditor?.document;
      if (document?.languageId !== "scoop" || document.uri.scheme !== "file") {
        void vscode.window.showErrorMessage(
          "Open a saved Scoop source file to debug.",
        );
        return;
      }
      await vscode.debug.startDebugging(
        vscode.workspace.getWorkspaceFolder(document.uri),
        initial(document.uri.fsPath),
      );
    }),
    vscode.commands.registerCommand("scoop.showDebugOutput", () =>
      output.show(),
    ),
  );
}
