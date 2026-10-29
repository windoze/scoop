import * as vscode from "vscode";
import { registerDebugging } from "./debug.ts";
import { folds } from "./folding.ts";
import { highlights, tokenModifiers, tokenTypes } from "./highlights.ts";
import { indentLines } from "./indentation.ts";
import { ScoopLanguageServer } from "./language-server.ts";
import { ScoopSyntax } from "./syntax.ts";

let languageServer: ScoopLanguageServer | undefined;

export async function activate(
  context: vscode.ExtensionContext,
): Promise<void> {
  registerDebugging(context);
  const syntax = await ScoopSyntax.load(context.asAbsolutePath("dist/assets"));
  const selector: vscode.DocumentSelector = { language: "scoop" };
  const legend = new vscode.SemanticTokensLegend(
    [...tokenTypes],
    [...tokenModifiers],
  );
  const parsed = (document: vscode.TextDocument) =>
    syntax.document(
      document.uri.toString(),
      document.getText(),
      document.version,
    );

  const reindent = (
    document: vscode.TextDocument,
    options: vscode.FormattingOptions,
    first?: number,
    last?: number,
    includeBlank = false,
  ) => {
    const state = parsed(document);
    return indentLines(
      state.source,
      syntax.indents.captures(state.tree.rootNode),
      options,
      first,
      last,
      includeBlank,
    ).map((edit) =>
      vscode.TextEdit.replace(
        new vscode.Range(edit.line, 0, edit.line, edit.length),
        edit.text,
      ),
    );
  };

  context.subscriptions.push(
    syntax,
    vscode.workspace.onDidChangeTextDocument((event) => {
      if (
        event.document.languageId === "scoop" &&
        event.contentChanges.length > 0
      ) {
        syntax.change(
          event.document.uri.toString(),
          event.contentChanges,
          event.document.getText(),
          event.document.version,
        );
      }
    }),
    vscode.workspace.onDidCloseTextDocument((document) =>
      syntax.close(document.uri.toString()),
    ),
    vscode.languages.registerDocumentSemanticTokensProvider(
      selector,
      {
        provideDocumentSemanticTokens(document, cancellation) {
          if (cancellation.isCancellationRequested) return;
          const state = parsed(document);
          const builder = new vscode.SemanticTokensBuilder(legend);
          const tokens = highlights(
            state.source,
            syntax.highlights.captures(state.tree.rootNode),
          );
          for (const token of tokens)
            builder.push(
              token.line,
              token.start,
              token.length,
              token.type,
              token.modifiers,
            );
          return cancellation.isCancellationRequested
            ? undefined
            : builder.build();
        },
      },
      legend,
    ),
    vscode.languages.registerFoldingRangeProvider(selector, {
      provideFoldingRanges(document, _context, cancellation) {
        if (cancellation.isCancellationRequested) return;
        const state = parsed(document);
        return folds(
          state.source,
          syntax.folds.captures(state.tree.rootNode),
        ).map(
          (fold) =>
            new vscode.FoldingRange(
              fold.start,
              fold.end,
              fold.kind === "comment"
                ? vscode.FoldingRangeKind.Comment
                : fold.kind === "imports"
                  ? vscode.FoldingRangeKind.Imports
                  : undefined,
            ),
        );
      },
    }),
    vscode.languages.registerOnTypeFormattingEditProvider(
      selector,
      {
        provideOnTypeFormattingEdits(
          document,
          position,
          _character,
          options,
          cancellation,
        ) {
          if (cancellation.isCancellationRequested) return;
          return reindent(
            document,
            options,
            position.line,
            position.line,
            true,
          );
        },
      },
      "\n",
      "}",
      "]",
      ")",
    ),
    vscode.languages.registerDocumentFormattingEditProvider(selector, {
      provideDocumentFormattingEdits(document, options, cancellation) {
        if (cancellation.isCancellationRequested) return;
        return reindent(document, options);
      },
    }),
    vscode.languages.registerDocumentRangeFormattingEditProvider(selector, {
      provideDocumentRangeFormattingEdits(
        document,
        range,
        options,
        cancellation,
      ) {
        if (cancellation.isCancellationRequested) return;
        const last =
          range.end.character === 0 && range.end.line > range.start.line
            ? range.end.line - 1
            : range.end.line;
        return reindent(document, options, range.start.line, last);
      },
    }),
  );
  languageServer = new ScoopLanguageServer(context);
  await languageServer.update();
}

export async function deactivate(): Promise<void> {
  await languageServer?.dispose();
  languageServer = undefined;
}
