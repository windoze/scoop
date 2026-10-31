import assert from "node:assert/strict";
import { after, test } from "node:test";
import { fileURLToPath } from "node:url";
import { runnables } from "../src/runnables.ts";
import { ScoopSyntax } from "../src/syntax.ts";

const syntax = await ScoopSyntax.load(
  fileURLToPath(new URL("../dist/assets/", import.meta.url)),
);
after(() => syntax.dispose());
let nextDocument = 0;

function targets(text: string) {
  const document = syntax.document(`runnable:${nextDocument++}`, text, 1);
  assert.equal(
    document.tree.rootNode.hasError,
    false,
    document.tree.rootNode.toString(),
  );
  return runnables(document).map((item) => ({
    ...item.target,
    text: text.slice(item.start, item.end),
  }));
}

test("lenses select only top-level ordinary main declarations", () => {
  assert.deepEqual(
    targets(
      [
        "// fun main() {}",
        "fun main(args: Array<String>): Int { return 0 }",
        "class Member { fun main() {} }",
        "fun wrapper() { fun main() {} }",
        'val sample = "fun main() {}"',
        "suspend fun main() {}",
        "fun <T> main() {}",
        "fun String.main() {}",
        "context(value: Int) fun main() {}",
      ].join("\n"),
    ),
    [{ kind: "main", name: "main", text: "main" }],
  );
  assert.deepEqual(targets("fun `main`(): Int = 0"), [
    { kind: "main", name: "main", text: "`main`" },
  ]);
});

test("test lenses support qualified annotations, aliases, private containers and Unicode names", () => {
  assert.deepEqual(
    targets(
      [
        "package demo./* package */`test-package`",
        "import scoop.test.`Test` as Case",
        "import scoop.test.Ignore",
        "@Case private class Cases {",
        "    @Case fun `检查 😀`() {}",
        '    @Ignore("later") @Case fun skipped() {}',
        "    @scoop./* marker */test.Test fun qualified() {}",
        "    @BeforeTest fun before() {}",
        "    @AfterTest fun after() {}",
        "    fun helper() {}",
        "    fun main() {}",
        "}",
      ].join("\r\n"),
    ),
    [
      {
        kind: "test",
        container: "demo.test-package.Cases",
        name: "检查 😀",
        text: "`检查 😀`",
      },
      {
        kind: "test",
        container: "demo.test-package.Cases",
        name: "skipped",
        text: "skipped",
      },
      {
        kind: "test",
        container: "demo.test-package.Cases",
        name: "qualified",
        text: "qualified",
      },
    ],
  );
});

test("test lenses exclude helpers, unsupported shapes and nested declarations", () => {
  assert.deepEqual(
    targets(
      [
        "import scoop.test.*",
        "@Test fun standalone() {}",
        "class Unmarked { @Test fun case() {} }",
        "@Test abstract class Abstract { @Test fun case() {} }",
        "@Test class Generic<T> { @Test fun case() {} }",
        "class Outer { @Test class Nested { @Test fun case() {} } }",
        "@Test object Singleton { @Test fun case() {} }",
        "@Test class Cases {",
        "    @Test fun <T> generic() {}",
        "    @Test fun parameter(value: Int) {}",
        "    @Test suspend fun async() {}",
        "    @Test fun String.extension() {}",
        "    @Test fun valid() { @Test fun local() {} }",
        "}",
      ].join("\n"),
    ),
    [{ kind: "test", container: "Cases", name: "valid", text: "valid" }],
  );
  assert.deepEqual(
    targets(
      [
        "annotation class Test",
        "@Test class Fake { @Test fun unrelated() {} }",
      ].join("\n"),
    ),
    [],
  );
});

test("lenses follow incremental declaration edits without retaining old targets", () => {
  const source = "fun main() {}";
  const document = syntax.document("runnable:editing", source, 1);
  assert.equal(runnables(document).length, 1);
  syntax.change(
    "runnable:editing",
    [{ rangeOffset: 4, rangeLength: 4, text: "helper" }],
    "fun helper() {}",
    2,
  );
  assert.deepEqual(runnables(document), []);
});
