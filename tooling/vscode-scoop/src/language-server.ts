import * as vscode from "vscode";
import {
  DidChangeConfigurationNotification,
  LanguageClient,
  RevealOutputChannelOn,
} from "vscode-languageclient/node";
import { analysisOptions, serverCommand } from "./server-settings.ts";

export class ScoopLanguageServer {
  private client: LanguageClient | undefined;
  private pending = Promise.resolve();
  private disposed = false;
  private readonly output = vscode.window.createOutputChannel("Scoop LSP", {
    log: true,
  });

  constructor(context: vscode.ExtensionContext) {
    context.subscriptions.push(
      this.output,
      vscode.commands.registerCommand("scoop.restartLanguageServer", () =>
        this.update(true),
      ),
      vscode.commands.registerCommand("scoop.showLanguageServerOutput", () =>
        this.output.show(),
      ),
      vscode.workspace.onDidGrantWorkspaceTrust(() => void this.update(true)),
      vscode.workspace.onDidChangeConfiguration((event) => {
        if (event.affectsConfiguration("scoop")) {
          void this.update(event.affectsConfiguration("scoop.server"));
        }
      }),
    );
  }

  update(restart = false): Promise<void> {
    // Serialize configuration changes and restart so only one process is active.
    this.pending = this.pending
      .then(async () => {
        const settings = vscode.workspace.getConfiguration("scoop");
        const enabled =
          !this.disposed &&
          vscode.workspace.isTrusted &&
          settings.get<boolean>("server.enabled", true);
        if (this.client && (restart || !enabled)) {
          await this.stop();
        }
        if (!enabled) return;
        if (this.client) {
          if (this.client.isRunning()) {
            await this.client.sendNotification(
              DidChangeConfigurationNotification.type,
              { settings: { scoop: this.options() } },
            );
          }
          return;
        }

        const command = await serverCommand(
          settings.get<string>("server.path", ""),
        );
        this.output.info(`Starting ${command} --stdio`);
        this.client = new LanguageClient(
          "scoop",
          "Scoop Language Server",
          { command, args: ["--stdio"] },
          {
            documentSelector: [
              { language: "scoop", scheme: "file" },
              { language: "scoop", scheme: "untitled" },
              { scheme: "file", pattern: "**/Cone.toml" },
            ],
            initializationOptions: () => this.options(),
            outputChannel: this.output,
            traceOutputChannel: this.output,
            revealOutputChannelOn: RevealOutputChannelOn.Never,
            initializationFailedHandler: () => false,
            middleware: {
              // Tree-sitter supplies folding, including when the server is offline.
              provideFoldingRanges: () => [],
            },
          },
        );
        try {
          await this.client.start();
        } catch {
          // LanguageClient reports startup errors itself. Release even a failed
          // client so correcting the executable path can start a fresh instance.
          await this.stop();
          this.output.info(
            "Check scoop.server.path, then run Scoop: Restart Language Server.",
          );
        }
      })
      .catch((error: unknown) => {
        const message = error instanceof Error ? error.message : String(error);
        this.output.error(message);
        if (this.disposed) return;
        void vscode.window
          .showErrorMessage(
            `Scoop LSP: ${message}. Check scoop.server.path and the installed toolchain.`,
            "Show Output",
          )
          .then((action) => {
            if (action === "Show Output") this.output.show();
          });
      });
    return this.pending;
  }

  private options() {
    return analysisOptions(
      vscode.workspace.getConfiguration().get("scoop", {}),
    );
  }

  private async stop() {
    const client = this.client;
    this.client = undefined;
    try {
      await client?.dispose();
    } catch (error) {
      // The client rejects stop after a failed start, but still releases its process.
      this.output.warn(`Language server cleanup: ${String(error)}`);
    }
  }

  dispose(): Promise<void> {
    this.disposed = true;
    return this.update();
  }
}
