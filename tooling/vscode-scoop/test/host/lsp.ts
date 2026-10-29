import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { writeFile } from "node:fs/promises";
import path from "node:path";
import { setTimeout } from "node:timers/promises";
import * as vscode from "vscode";

async function eventually(
  check: () => Promise<boolean> | boolean,
  description: string,
) {
  const deadline = Date.now() + 20000;
  while (!(await check())) {
    assert.ok(Date.now() < deadline, `Timed out: ${description}`);
    await setTimeout(50);
  }
}

function at(document: vscode.TextDocument, text: string, after = false) {
  const offset = document.getText().indexOf(text);
  assert.ok(offset >= 0, text);
  return document.positionAt(offset + (after ? text.length : 0));
}

async function replace(
  document: vscode.TextDocument,
  before: string,
  after: string,
) {
  const edit = new vscode.WorkspaceEdit();
  edit.replace(
    document.uri,
    new vscode.Range(at(document, before), at(document, before, true)),
    after,
  );
  assert.ok(await vscode.workspace.applyEdit(edit));
}

async function write(uri: vscode.Uri, text: string) {
  await vscode.workspace.fs.writeFile(uri, new TextEncoder().encode(text));
}

async function hover(document: vscode.TextDocument, name: string) {
  return vscode.commands.executeCommand<vscode.Hover[]>(
    "vscode.executeHoverProvider",
    document.uri,
    at(document, name),
  );
}

export async function runLanguageServerTests() {
  const folder = vscode.workspace.workspaceFolders?.[0];
  assert.ok(folder);
  const project = vscode.Uri.joinPath(folder.uri, "language project");
  await vscode.workspace.fs.createDirectory(
    vscode.Uri.joinPath(project, "src"),
  );
  const manifestUri = vscode.Uri.joinPath(project, "Cone.toml");
  const manifestText =
    'schema = 1\n[cone]\ngroup = "test"\nname = "vscode"\nversion = "1.0.0"\nkind = "library"\n';
  await write(manifestUri, manifestText);
  const apiUri = vscode.Uri.joinPath(project, "src/api.scoop");
  const apiText =
    "package editor\n/// Returns the supplied flag.\npublic fun choose(flag: Boolean): Boolean = flag\n";
  await write(apiUri, apiText);
  const diskUri = vscode.Uri.joinPath(project, "src/disk.scoop");
  const diskText =
    "package editor\nfun diskFlag(flag: Boolean): Boolean = true\n";
  await write(diskUri, diskText);
  const useUri = vscode.Uri.joinPath(project, "src/use.scoop");
  await write(
    useUri,
    "package editor\nfun use(): Boolean = choose(true)\nfun inspect(𐐀flag: Boolean): String = 𐐀flag.toString()\nfun fromDisk(): Boolean = diskFlag(true)\n",
  );
  const document = await vscode.workspace.openTextDocument(useUri);
  await vscode.window.showTextDocument(document);
  const settings = () => vscode.workspace.getConfiguration("scoop");
  const pairedServer = settings().get<string>("server.path");
  assert.ok(pairedServer);
  const pm = path.join(
    path.dirname(pairedServer),
    process.platform === "win32" ? "scoop-pm.exe" : "scoop-pm",
  );
  execFileSync(
    pm,
    [
      "resolve",
      project.fsPath,
      "--offline",
      "--sysroot",
      settings().get<string>("sysroot") ?? "",
    ],
    { stdio: "pipe" },
  );
  const update = (key: string, value: unknown) =>
    settings().update(key, value, vscode.ConfigurationTarget.Workspace);
  await update("server.enabled", true);
  await vscode.commands.executeCommand("scoop.restartLanguageServer");
  await eventually(
    async () => Boolean((await hover(document, "choose"))?.length),
    "semantic hover after startup",
  );
  const documentation = (await hover(document, "choose"))
    ?.flatMap((item) =>
      item.contents.map((content) =>
        typeof content === "string" ? content : content.value,
      ),
    )
    .join("\n");
  assert.match(documentation ?? "", /Returns the supplied flag/);

  const definitions = await vscode.commands.executeCommand<
    (vscode.Location | vscode.LocationLink)[]
  >("vscode.executeDefinitionProvider", useUri, at(document, "choose"));
  assert.ok(
    definitions?.some(
      (location) =>
        ("targetUri" in location
          ? location.targetUri
          : location.uri
        ).toString() === apiUri.toString(),
    ),
  );
  const references = await vscode.commands.executeCommand<vscode.Location[]>(
    "vscode.executeReferenceProvider",
    useUri,
    at(document, "choose"),
  );
  assert.ok(
    references?.some(
      (location) => location.uri.toString() === useUri.toString(),
    ),
  );

  const completion =
    await vscode.commands.executeCommand<vscode.CompletionList>(
      "vscode.executeCompletionItemProvider",
      useUri,
      at(document, "𐐀flag.", true),
      ".",
    );
  assert.ok(
    completion?.items.some(
      (item) =>
        (typeof item.label === "string" ? item.label : item.label.label) ===
        "toString",
    ),
  );
  const signature = await vscode.commands.executeCommand<vscode.SignatureHelp>(
    "vscode.executeSignatureHelpProvider",
    useUri,
    at(document, "choose(", true),
  );
  assert.match(
    signature?.signatures[0].label ?? "",
    /choose\(flag: Boolean\): Boolean/,
  );
  const symbols = await vscode.commands.executeCommand<vscode.DocumentSymbol[]>(
    "vscode.executeDocumentSymbolProvider",
    useUri,
  );
  assert.ok(symbols?.some((symbol) => symbol.name === "inspect"));
  const workspaceSymbols = await vscode.commands.executeCommand<
    vscode.SymbolInformation[]
  >("vscode.executeWorkspaceSymbolProvider", "choose");
  assert.ok(
    workspaceSymbols?.some(
      (symbol) => symbol.location.uri.toString() === apiUri.toString(),
    ),
  );
  const selections = await vscode.commands.executeCommand<
    vscode.SelectionRange[]
  >("vscode.executeSelectionRangeProvider", useUri, [at(document, "choose")]);
  assert.ok(selections?.[0].parent);

  const api = await vscode.workspace.openTextDocument(apiUri);
  const rename = await vscode.commands.executeCommand<vscode.WorkspaceEdit>(
    "vscode.executeDocumentRenameProvider",
    useUri,
    at(document, "choose"),
    "selectFlag",
  );
  assert.ok(rename && rename.size === 2);
  assert.ok(await vscode.workspace.applyEdit(rename));
  assert.match(document.getText(), /selectFlag\(true\)/);
  assert.match(api.getText(), /fun selectFlag/);

  await replace(document, "selectFlag(true)", 'selectFlag("wrong")');
  await eventually(
    () =>
      vscode.languages
        .getDiagnostics(useUri)
        .some((item) => /Boolean/.test(item.message)),
    "type diagnostic for an unsaved edit",
  );
  await replace(document, 'selectFlag("wrong")', "selectFlag(true)");
  await eventually(
    () => vscode.languages.getDiagnostics(useUri).length === 0,
    "cleared diagnostic after correction",
  );

  // Cone overlays and live configuration must reach the server without restarting.
  const manifest = await vscode.workspace.openTextDocument(manifestUri);
  await replace(manifest, 'kind = "library"', 'kind = "invalid"');
  await eventually(
    () => vscode.languages.getDiagnostics(manifestUri).length > 0,
    "unsaved Cone.toml diagnostic",
  );
  await replace(manifest, 'kind = "invalid"', 'kind = "library"');
  await eventually(
    () => vscode.languages.getDiagnostics(manifestUri).length === 0,
    "corrected Cone.toml overlay",
  );
  await update("target", "invalid-target");
  await eventually(
    () =>
      vscode.languages
        .getDiagnostics(manifestUri)
        .some((item) => /invalid-target/.test(item.message)),
    "changed analysis target",
  );
  await update("target", "");
  await eventually(
    () => vscode.languages.getDiagnostics(manifestUri).length === 0,
    "restored default analysis target",
  );

  // A source without an editor buffer is picked up by the dynamic watcher.
  await writeFile(
    diskUri.fsPath,
    diskText.replace("flag: Boolean", "flag: String"),
  );
  await eventually(
    () => vscode.languages.getDiagnostics(useUri).length > 0,
    "watched dependency edit",
  );
  await writeFile(diskUri.fsPath, diskText);
  await eventually(
    () => vscode.languages.getDiagnostics(useUri).length === 0,
    "watched dependency correction",
  );

  const untitled = await vscode.workspace.openTextDocument({
    language: "scoop",
    content: "fun pending() {",
  });
  await vscode.window.showTextDocument(untitled);
  await eventually(
    () => vscode.languages.getDiagnostics(untitled.uri).length > 0,
    "untitled syntax diagnostics",
  );
  const actions = await vscode.commands.executeCommand<vscode.CodeAction[]>(
    "vscode.executeCodeActionProvider",
    untitled.uri,
    new vscode.Range(0, 0, 0, 15),
    vscode.CodeActionKind.QuickFix.value,
    10,
  );
  const fix = actions?.find((action) => action.edit);
  assert.ok(fix?.edit, "missing delimiter quickfix");
  assert.ok(await vscode.workspace.applyEdit(fix.edit));
  await eventually(
    () => vscode.languages.getDiagnostics(untitled.uri).length === 0,
    "quickfix clears syntax error",
  );

  await vscode.commands.executeCommand("scoop.restartLanguageServer");
  await eventually(
    async () => Boolean((await hover(document, "selectFlag"))?.length),
    "restart resynchronizes open documents",
  );
  const server = settings().get<string>("server.path");
  await update(
    "server.path",
    vscode.Uri.joinPath(project, "missing-scoop-lsp").fsPath,
  );
  await vscode.commands.executeCommand("scoop.restartLanguageServer");
  assert.ok(
    (
      await vscode.commands.executeCommand<vscode.SemanticTokens>(
        "vscode.provideDocumentSemanticTokens",
        useUri,
      )
    )?.data.length,
    "highlighting survives a missing server",
  );
  await update("server.path", server);
  await vscode.commands.executeCommand("scoop.restartLanguageServer");
  await eventually(
    async () => Boolean((await hover(document, "selectFlag"))?.length),
    "recovery after server path correction",
  );
  await update("server.enabled", false);
  await vscode.commands.executeCommand("scoop.restartLanguageServer");
  assert.deepEqual(await hover(document, "selectFlag"), []);
  assert.equal(vscode.languages.getDiagnostics(untitled.uri).length, 0);
  console.log(
    "Scoop LSP Extension Host: completion, hover, signature, navigation, symbols, references, rename, diagnostics, quickfix, Cone overlays, configuration, watchers, restart and fallback passed.",
  );
}
