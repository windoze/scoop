import assert from "node:assert/strict";
import * as vscode from "vscode";
import { runLanguageServerTests } from "./lsp.ts";

const formatting = { tabSize: 4, insertSpaces: true };

async function open(name: string, text: string) {
  const folder = vscode.workspace.workspaceFolders?.[0];
  assert.ok(folder);
  const uri = vscode.Uri.joinPath(folder.uri, name);
  await vscode.workspace.fs.writeFile(uri, new TextEncoder().encode(text));
  const document = await vscode.workspace.openTextDocument(uri);
  await vscode.window.showTextDocument(document);
  assert.equal(document.languageId, "scoop");
  return document;
}

async function semanticTokens(document: vscode.TextDocument) {
  const legend =
    await vscode.commands.executeCommand<vscode.SemanticTokensLegend>(
      "vscode.provideDocumentSemanticTokensLegend",
      document.uri,
    );
  const result = await vscode.commands.executeCommand<vscode.SemanticTokens>(
    "vscode.provideDocumentSemanticTokens",
    document.uri,
  );
  assert.ok(legend);
  assert.ok(result?.data.length);
  const tokens: { text: string; type: string }[] = [];
  let line = 0;
  let column = 0;
  for (let i = 0; i < result.data.length; i += 5) {
    column =
      result.data[i] === 0 ? column + result.data[i + 1] : result.data[i + 1];
    line += result.data[i];
    const length = result.data[i + 2];
    assert.ok(column + length <= document.lineAt(line).text.length);
    tokens.push({
      text: document.getText(
        new vscode.Range(line, column, line, column + length),
      ),
      type: legend.tokenTypes[result.data[i + 3]],
    });
  }
  return tokens;
}

export async function run(): Promise<void> {
  const document = await open(
    "editor.scoop",
    [
      "import demo.One",
      "import demo.Two",
      "/// Example.",
      "fun paint(𐐀x: String) {",
      'val context = f"😀 ${𐐀x}"',
      "if (true) {",
      "paint(context)",
      "}",
      "}",
    ].join("\n"),
  );
  const extension = vscode.extensions.getExtension(
    "scoop.scoop-language-support",
  );
  assert.ok(extension);
  await extension.activate();
  assert.ok(extension.isActive);
  assert.equal(
    vscode.workspace.getConfiguration("editor", document).get("formatOnType"),
    true,
  );

  const tokens = await semanticTokens(document);
  for (const expected of [
    { text: "paint", type: "function" },
    { text: "𐐀x", type: "parameter" },
    { text: "String", type: "type" },
    { text: "context", type: "variable" },
  ])
    assert.ok(
      tokens.some(
        (token) => token.text === expected.text && token.type === expected.type,
      ),
      JSON.stringify(expected),
    );

  const folding = await vscode.commands.executeCommand<vscode.FoldingRange[]>(
    "vscode.executeFoldingRangeProvider",
    document.uri,
  );
  assert.ok(folding?.some((range) => range.start === 0 && range.end === 1));
  assert.ok(folding.some((range) => range.start === 3 && range.end === 7));
  assert.ok(folding.some((range) => range.start === 5 && range.end === 6));

  const selected = await vscode.commands.executeCommand<vscode.TextEdit[]>(
    "vscode.executeFormatRangeProvider",
    document.uri,
    new vscode.Range(4, 0, 5, 0),
    formatting,
  );
  assert.ok(selected?.length);
  assert.ok(
    selected.every(
      (edit) => edit.range.start.line === 4 && edit.range.end.line === 4,
    ),
  );
  const edits = await vscode.commands.executeCommand<vscode.TextEdit[]>(
    "vscode.executeFormatDocumentProvider",
    document.uri,
    formatting,
  );
  assert.ok(edits?.length);
  for (const edit of edits) {
    assert.equal(edit.range.start.character, 0);
    assert.equal(edit.range.start.line, edit.range.end.line);
    assert.match(document.getText(edit.range), /^[\t ]*$/);
    assert.match(edit.newText, /^[\t ]*$/);
  }
  const workspaceEdit = new vscode.WorkspaceEdit();
  workspaceEdit.set(document.uri, edits);
  assert.ok(await vscode.workspace.applyEdit(workspaceEdit));
  assert.equal(document.lineAt(4).text, '    val context = f"😀 ${𐐀x}"');
  assert.equal(document.lineAt(6).text, "        paint(context)");
  assert.equal(document.lineAt(7).text, "    }");
  const repeated = await vscode.commands.executeCommand<vscode.TextEdit[]>(
    "vscode.executeFormatDocumentProvider",
    document.uri,
    formatting,
  );
  assert.deepEqual(repeated ?? [], []);

  const rename = new vscode.WorkspaceEdit();
  const offset = document.getText().indexOf("paint");
  rename.replace(
    document.uri,
    new vscode.Range(
      document.positionAt(offset),
      document.positionAt(offset + 5),
    ),
    "绘制",
  );
  assert.ok(await vscode.workspace.applyEdit(rename));
  assert.ok(
    (await semanticTokens(document)).some(
      (token) => token.text === "绘制" && token.type === "function",
    ),
  );

  const incomplete = await open("incomplete.scoop", "fun unfinished() {\n");
  const onType = await vscode.commands.executeCommand<vscode.TextEdit[]>(
    "vscode.executeFormatOnTypeProvider",
    incomplete.uri,
    new vscode.Position(1, 0),
    "\n",
    formatting,
  );
  assert.equal(onType?.length, 1);
  assert.equal(onType[0].newText, "    ");
  assert.equal(onType[0].range.start.line, 1);

  await vscode.workspace.saveAll();
  await vscode.commands.executeCommand("workbench.action.closeAllEditors");
  console.log(
    "Scoop Extension Host: activation, UTF-16 tokens, folding, range/document/on-type indentation and incremental edits passed.",
  );
  await runLanguageServerTests();
}
