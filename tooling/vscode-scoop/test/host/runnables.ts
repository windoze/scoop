import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { promisify } from "node:util";
import * as vscode from "vscode";

const execute = promisify(execFile);

async function write(root: vscode.Uri, name: string, text: string) {
  const uri = vscode.Uri.joinPath(root, name);
  await vscode.workspace.fs.writeFile(uri, new TextEncoder().encode(text));
  return uri;
}

async function lenses(uri: vscode.Uri) {
  const document = await vscode.workspace.openTextDocument(uri);
  await vscode.window.showTextDocument(document);
  const result = await vscode.commands.executeCommand<vscode.CodeLens[]>(
    "vscode.executeCodeLensProvider",
    uri,
  );
  assert.ok(result);
  return result;
}

async function press(lens: vscode.CodeLens) {
  assert.ok(lens.command);
  return vscode.commands.executeCommand(
    lens.command.command,
    ...(lens.command.arguments ?? []),
  );
}

export async function runRunnableTests(): Promise<void> {
  const folder = vscode.workspace.workspaceFolders?.[0];
  assert.ok(folder);
  const project = vscode.Uri.joinPath(folder.uri, "lens project");
  await vscode.workspace.fs.createDirectory(
    vscode.Uri.joinPath(project, "src"),
  );
  await vscode.workspace.fs.createDirectory(
    vscode.Uri.joinPath(project, "tests"),
  );
  await write(
    project,
    "Cone.toml",
    [
      "schema = 1",
      "[cone]",
      'group = "test"',
      'name = "lenses"',
      'version = "0.1.0"',
      'kind = "executable"',
      "[test-dependencies]",
      '"scoop:scoop.test" = "0.1.0"',
    ].join("\n"),
  );
  const main = await write(
    project,
    "src/main.scoop",
    ["package test.lenses", "fun main(): Int { return result() }"].join("\n"),
  );
  await write(
    project,
    "src/helper.scoop",
    "package test.lenses\nfun result(): Int = 0\n",
  );
  const tests = await write(
    project,
    "tests/cases.scoop",
    [
      "package test.lenses",
      "import scoop.test.*",
      "import scoop.test.Test as Case",
      "@Case private class Cases {",
      '    @BeforeTest fun before() { println("before") }',
      '    @Case fun check() { println("selected") }',
      '    @Case fun checkMore() { println("wrong test") }',
      '    @Ignore("explicit only") @Case fun ignored() { println("ignored") }',
      '    @AfterTest fun after() { println("after") }',
      "}",
    ].join("\n"),
  );
  const mainLenses = await lenses(main);
  assert.deepEqual(
    mainLenses.map((lens) => lens.command?.title),
    ["Run", "Debug"],
  );
  assert.ok(mainLenses.every((lens) => lens.range.start.line === 1));
  const testLenses = await lenses(tests);
  assert.deepEqual(
    testLenses.map((lens) => lens.range.start.line),
    [5, 5, 6, 6, 7, 7],
  );

  // Let the real provider save and build, then inspect its launch configuration
  // without requiring native debugger permissions on the Extension Host machine.
  const resolved: vscode.DebugConfiguration[] = [];
  const interception = vscode.debug.registerDebugConfigurationProvider(
    "scoop",
    {
      resolveDebugConfigurationWithSubstitutedVariables(_folder, config) {
        resolved.push(config);
        return undefined;
      },
    },
  );
  try {
    // Another editor is active: the clicked lens must still select its own Cone.
    assert.equal(await press(mainLenses[1]), false);
    assert.equal(resolved.length, 1);
    const debug = resolved[0];
    assert.equal(debug.type, "scoop");
    assert.equal(debug.request, "launch");
    assert.equal(debug.cwd, project.fsPath);
    assert.equal(debug.noDebug, false);
    assert.equal(debug.build, undefined);
    assert.ok(
      debug.initCommands.some((command: string) =>
        command.includes("scoop_lldb.py"),
      ),
    );
    assert.equal(
      (
        await execute(debug.program, debug.args, {
          cwd: debug.cwd,
          timeout: 30000,
        })
      ).stdout,
      "",
    );

    assert.equal(await press(mainLenses[0]), false);
    assert.equal(resolved.length, 2);
    assert.equal(resolved[1].noDebug, true);

    assert.equal(await press(testLenses[1]), false);
    assert.equal(resolved.length, 3);
    const selected = resolved[2];
    assert.equal(selected.cwd, project.fsPath);
    assert.equal(selected.noDebug, false);
    assert.deepEqual(selected.args, ["--scoop-test-cases", "0"]);
    assert.equal(
      (
        await execute(selected.program, selected.args, {
          cwd: selected.cwd,
          timeout: 30000,
        })
      ).stdout,
      "before\nselected\nafter\n",
    );

    assert.equal(await press(testLenses[4]), false);
    assert.equal(resolved.length, 4);
    const ignored = resolved[3];
    assert.equal(ignored.noDebug, true);
    assert.deepEqual(ignored.args, ["--scoop-test-cases", "2"]);
    assert.equal(
      (
        await execute(ignored.program, ignored.args, {
          cwd: ignored.cwd,
          timeout: 30000,
        })
      ).stdout,
      "before\nignored\nafter\n",
    );

    const document = await vscode.workspace.openTextDocument(main);
    const edit = new vscode.WorkspaceEdit();
    edit.insert(
      main,
      document.positionAt(document.getText().length),
      "\nfun broken(\n",
    );
    assert.ok(await vscode.workspace.applyEdit(edit));
    assert.equal(document.isDirty, true);
    assert.equal(await press(mainLenses[1]), false);
    assert.equal(
      document.isDirty,
      false,
      "launch must save the source before compiling",
    );
    assert.equal(
      resolved.length,
      4,
      "a failed build must not reach adapter launch with the previous program",
    );
    await assert.rejects(async () =>
      vscode.workspace.fs.stat(
        vscode.Uri.joinPath(folder.uri, ".vscode/launch.json"),
      ),
    );
  } finally {
    interception.dispose();
    await vscode.commands.executeCommand("workbench.action.closeAllEditors");
  }
  console.log(
    "Scoop Extension Host: main/test CodeLens, automatic configurations, Cone builds, exact test execution and failed-build handling passed.",
  );
}
