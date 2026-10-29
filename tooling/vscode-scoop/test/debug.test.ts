import assert from "node:assert/strict";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import {
  buildArguments,
  buildProgram,
  formatterCommand,
  testArguments,
  toolCommand,
} from "../src/debug-settings.ts";

test("debug tool discovery uses the host's installed command names", async () => {
  assert.equal(
    path.basename(await toolCommand("scoop", "")),
    process.platform === "win32" ? "sco.exe" : "scoop",
  );
  assert.equal(
    path.basename(await toolCommand("scoop-lldb-dap", "")),
    process.platform === "win32" ? "scoop-lldb-dap.exe" : "scoop-lldb-dap",
  );
  assert.equal(
    await toolCommand("scoop", "custom/compiler"),
    "custom/compiler",
  );
  assert.equal(
    await toolCommand("scoop-lldb-dap", "custom/lldb-dap"),
    "custom/lldb-dap",
  );
});

test("debug builds use full information and test filters reach the runner", () => {
  assert.deepEqual(
    buildArguments(
      { input: "path with spaces", tests: true, profile: "release" },
      "/sysroot",
    ),
    [
      "test",
      "path with spaces",
      "--no-run",
      "--profile",
      "release",
      "--debug-info",
      "full",
      "--message-format",
      "json",
      "--sysroot",
      "/sysroot",
    ],
  );
  assert.deepEqual(testArguments({ tests: true, filter: "中文 test" }), [
    "--scoop-test-filter",
    "中文 test",
  ]);
  assert.deepEqual(testArguments({ tests: false, filter: "unused" }), []);
  assert.equal(
    formatterCommand('/with spaces/a"b\\c.py'),
    'command script import "/with spaces/a\\"b\\\\c.py"',
  );
  assert.throws(() => formatterCommand("bad\ncommand"));
});

test("debug build consumes the executable record and keeps argument boundaries", async () => {
  const directory = await mkdtemp(path.join(os.tmpdir(), "scoop debug "));
  try {
    const script = path.join(directory, "build.mjs");
    await writeFile(
      script,
      `
      console.log('tool output');
      console.error(JSON.stringify({schema: 1, kind: 'diagnostic', message: 'warning'}));
      console.error(JSON.stringify({schema: 1, kind: 'executable', output: process.argv[2]}));
    `,
    );
    const lines: string[] = [];
    const executable = path.join(directory, "program ' $value");
    assert.equal(
      await buildProgram(
        process.execPath,
        [script, executable],
        directory,
        (line) => lines.push(line),
      ),
      executable,
    );
    assert.ok(lines.includes("tool output"));
    await writeFile(
      script,
      "console.error(JSON.stringify({schema:1,kind:'executable',output:'/wrong'})); process.exit(1);",
    );
    await assert.rejects(
      buildProgram(process.execPath, [script], directory, () => {}),
      /build failed/,
    );
    await writeFile(
      script,
      "for(const output of ['/one','/two']) console.error(JSON.stringify({schema:1,kind:'executable',output}));",
    );
    await assert.rejects(
      buildProgram(process.execPath, [script], directory, () => {}),
      /Select one executable/,
    );
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});
