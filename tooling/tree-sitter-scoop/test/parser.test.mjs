import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { after, test } from "node:test";
import { errors, parser, query, root, sources } from "./helpers.mjs";

after(() => parser.delete());

for await (const file of sources(path.join(root, "test/fixtures"))) {
  test(`parse ${path.basename(file)}`, async () => {
    const tree = parser.parse(await readFile(file, "utf8"));
    try {
      assert.deepEqual(errors(tree), []);
    } finally {
      tree.delete();
    }
  });
}

test("core and standard library sources stay compatible with the editor grammar", async () => {
  const failures = [];
  let count = 0;
  for await (const file of sources(path.resolve(root, "../../sysroot/src"))) {
    const tree = parser.parse(await readFile(file, "utf8"));
    if (tree.rootNode.hasError)
      failures.push(
        `${path.relative(root, file)}: ${errors(tree).slice(0, 3).join("; ")}`,
      );
    tree.delete();
    count++;
  }
  assert.ok(
    count > 0,
    "the source compatibility test must read real library files",
  );
  assert.equal(
    failures.length,
    0,
    `${failures.length} of ${count} library sources failed:\n${failures.slice(0, 10).join("\n")}`,
  );
});

test("all shipped queries compile", async () => {
  for (const name of ["highlights", "folds", "indents"]) {
    const compiled = await query(name);
    assert.ok(compiled.captureNames.length > 0);
    compiled.delete();
  }
});

test("an incomplete declaration does not hide the following function", () => {
  const tree = parser.parse(
    "fun broken() { val x = }\nfun intact(): Int = 42\n",
  );
  try {
    assert.equal(tree.rootNode.hasError, true);
    assert.ok(
      tree.rootNode
        .descendantsOfType("function_declaration")
        .some((node) => node.childForFieldName("name")?.text === "intact"),
    );
  } finally {
    tree.delete();
  }
});

for (const source of [
  'val x = "unterminated\n',
  'val x = f"${value"\n',
  "fun missing() {\n",
  "val x = 1__2\n",
  'val x = "first\nsecond"\n',
  'val x = f"first\n${second}"\n',
]) {
  test(`recover malformed syntax: ${JSON.stringify(source)}`, () => {
    const tree = parser.parse(source);
    try {
      assert.equal(tree.rootNode.hasError, true);
    } finally {
      tree.delete();
    }
  });
}
