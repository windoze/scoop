import assert from "node:assert/strict";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import {
  buildArguments,
  buildProgram,
  formatterCommand,
  sourceBuildInput,
  sourceLaunchConfiguration,
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
    assert.deepEqual(
      await buildProgram(
        process.execPath,
        [script, executable],
        directory,
        (line) => lines.push(line),
      ),
      { program: executable, testDirectory: undefined },
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

test("source launches select the closest Cone, or an independent source file", async () => {
  const root = await mkdtemp(path.join(os.tmpdir(), "scoop lens "));
  try {
    const standalone = path.join(root, "standalone.scoop");
    await writeFile(standalone, "fun main() {}");
    assert.deepEqual(await sourceBuildInput(standalone), {
      input: standalone,
      cwd: root,
      manifest: false,
    });
    await assert.rejects(
      sourceLaunchConfiguration(standalone, {
        kind: "test",
        container: "Cases",
        name: "case",
      }),
      /require a Cone.toml/,
    );
    await writeFile(
      path.join(root, "Cone.toml"),
      '[workspace]\nmembers = ["member"]\n',
    );
    const member = path.join(root, "member");
    await mkdir(path.join(member, "tests", "nested"), { recursive: true });
    await writeFile(
      path.join(member, "Cone.toml"),
      '[cone]\nname = "member"\n',
    );
    const file = path.join(member, "tests", "nested", "检查.scoop");
    await writeFile(file, "");
    const config = await sourceLaunchConfiguration(file, {
      kind: "test",
      container: "demo.Cases",
      name: "检查 😀",
    });
    assert.equal(config.cwd, member);
    assert.equal(config.noDebug, false);
    assert.equal(config.type, "scoop");
    assert.equal(config.request, "launch");
    assert.deepEqual(config.build, {
      input: member,
      tests: true,
      testCase: {
        path: "tests/nested/检查.scoop",
        container: "demo.Cases",
        name: "检查 😀",
      },
    });
    const run = await sourceLaunchConfiguration(
      file,
      { kind: "main", name: "main" },
      true,
    );
    assert.equal(run.noDebug, true);
    assert.equal(run.build.input, member);
    assert.equal(run.build.tests, undefined);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("a test lens selects one directory index including ignored and private cases", async () => {
  const selected = {
    path: "tests/a.scoop",
    container: "demo.Cases",
    name: "check",
  };
  const directory = {
    containers: [
      {
        name: "demo.Cases",
        cases: [
          {
            name: "ignored",
            location: { path: "tests/a.scoop" },
            ignore_reason: "later",
          },
          { name: "check", location: { path: "tests/a.scoop" } },
          { name: "checkMore", location: { path: "tests/a.scoop" } },
        ],
      },
      { name: "demo.Empty", cases: [] },
      {
        name: "demo.Cases",
        cases: [{ name: "check", location: { path: "tests/b.scoop" } }],
      },
    ],
  };
  const root = await mkdtemp(path.join(os.tmpdir(), "scoop directory "));
  try {
    const script = path.join(root, "build.mjs");
    await writeFile(
      script,
      `console.error(${JSON.stringify(
        JSON.stringify({
          schema: 1,
          kind: "executable",
          output: "program",
          test_directory: directory,
        }),
      )});`,
    );
    const result = await buildProgram(
      process.execPath,
      [script],
      root,
      () => {},
    );
    const build = { tests: true, filter: "ignored", testCase: selected };
    assert.deepEqual(testArguments(build, result), ["--scoop-test-cases", "1"]);
    assert.deepEqual(
      testArguments(
        { ...build, testCase: { ...selected, name: "ignored" } },
        result,
      ),
      ["--scoop-test-cases", "0"],
    );
    assert.deepEqual(
      testArguments(
        { ...build, testCase: { ...selected, path: "tests/b.scoop" } },
        result,
      ),
      ["--scoop-test-cases", "3"],
    );
    assert.throws(
      () => testArguments(build),
      /did not return a test directory/,
    );
    assert.throws(
      () =>
        testArguments(
          { ...build, testCase: { ...selected, name: "missing" } },
          result,
        ),
      /Cannot find one test/,
    );
    assert.throws(
      () =>
        testArguments(build, {
          program: "program",
          testDirectory: {
            containers: [directory.containers[0], directory.containers[0]],
          },
        }),
      /Cannot find one test/,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("cancelled builds reject without returning an executable", async () => {
  const cancellation = new AbortController();
  cancellation.abort();
  await assert.rejects(
    buildProgram(
      process.execPath,
      ["-e", "setInterval(() => {}, 1000)"],
      os.tmpdir(),
      () => {},
      cancellation.signal,
    ),
    { name: "AbortError" },
  );
});
