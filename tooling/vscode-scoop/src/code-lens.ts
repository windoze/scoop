import * as vscode from "vscode";
import { launchSource } from "./debug.ts";
import { type RunnableTarget, runnables } from "./runnables.ts";
import type { ScoopSyntax } from "./syntax.ts";

export function registerCodeLenses(
  context: vscode.ExtensionContext,
  syntax: ScoopSyntax,
): void {
  context.subscriptions.push(
    vscode.languages.registerCodeLensProvider(
      { language: "scoop", scheme: "file" },
      {
        provideCodeLenses(document, cancellation) {
          if (cancellation.isCancellationRequested) return;
          const state = syntax.document(
            document.uri.toString(),
            document.getText(),
            document.version,
          );
          return runnables(state).flatMap((runnable) => {
            const range = new vscode.Range(
              document.positionAt(runnable.start),
              document.positionAt(runnable.end),
            );
            return ["Run", "Debug"].map(
              (title) =>
                new vscode.CodeLens(range, {
                  title,
                  command:
                    title === "Run" ? "scoop.runTarget" : "scoop.debugTarget",
                  arguments: [document.uri.toString(), runnable.target],
                }),
            );
          });
        },
      },
    ),
    ...(["run", "debug"] as const).map((mode) =>
      vscode.commands.registerCommand(
        mode === "run" ? "scoop.runTarget" : "scoop.debugTarget",
        (uri: string, target: RunnableTarget) =>
          launchSource(vscode.Uri.parse(uri), target, mode === "run"),
      ),
    ),
  );
}
