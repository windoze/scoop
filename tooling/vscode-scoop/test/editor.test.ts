import assert from "node:assert/strict";
import { after, test } from "node:test";
import { fileURLToPath } from "node:url";
import { Parser } from "web-tree-sitter";
import { folds } from "../src/folding.ts";
import { highlights, tokenTypes } from "../src/highlights.ts";
import { type IndentEdit, indentLines } from "../src/indentation.ts";
import { type Edit, ScoopSyntax, type SyntaxDocument } from "../src/syntax.ts";
import { SourceText } from "../src/text.ts";

const syntax = await ScoopSyntax.load(
  fileURLToPath(new URL("../dist/assets/", import.meta.url)),
);
after(() => syntax.dispose());
let nextDocument = 0;
function document(text: string): SyntaxDocument {
  const result = syntax.document(`test:${nextDocument++}`, text, 1);
  assert.equal(
    result.tree.rootNode.hasError,
    false,
    result.tree.rootNode.toString(),
  );
  return result;
}

function tokensFor(state: SyntaxDocument) {
  return highlights(
    state.source,
    syntax.highlights.captures(state.tree.rootNode),
  );
}

function typeAt(state: SyntaxDocument, offset: number): string | undefined {
  const position = state.source.positionAt(offset);
  const token = tokensFor(state).find(
    (token) =>
      token.line === position.row &&
      token.start <= position.column &&
      token.start + token.length > position.column,
  );
  return token ? tokenTypes[token.type] : undefined;
}

function reindent(
  state: SyntaxDocument,
  first?: number,
  last?: number,
  insertSpaces = true,
) {
  return indentLines(
    state.source,
    syntax.indents.captures(state.tree.rootNode),
    { tabSize: 4, insertSpaces },
    first,
    last,
  );
}

function applyIndentEdits(source: SourceText, edits: IndentEdit[]): string {
  let result = source.text;
  for (const edit of [...edits].reverse()) {
    const start = source.lineStarts[edit.line];
    result =
      result.slice(0, start) + edit.text + result.slice(start + edit.length);
  }
  return result;
}

test("highlights syntax roles, documentation and interpolation without overlapping tokens", () => {
  const text = [
    "/// Paint a value.",
    "@Mark",
    "fun paint(color: String) {",
    '    val context = f"雪 ${color} 😀"',
    '    val literal = "${color}"',
    "    paint(context)",
    "}",
  ].join("\n");
  const state = document(text);
  assert.equal(typeAt(state, text.indexOf("paint")), "function");
  assert.equal(typeAt(state, text.indexOf("color")), "parameter");
  assert.equal(typeAt(state, text.indexOf("String")), "type");
  assert.equal(typeAt(state, text.indexOf("Mark")), "decorator");
  assert.equal(typeAt(state, text.indexOf("雪")), "string");
  assert.equal(typeAt(state, text.indexOf("${color}") + 2), "variable");
  assert.equal(typeAt(state, text.lastIndexOf("${color}") + 2), "string");
  assert.equal(typeAt(state, text.lastIndexOf("context")), "variable");
  const tokens = tokensFor(state);
  assert.ok(tokens[0].modifiers & (1 << 3));
  for (let index = 0; index < tokens.length; index++) {
    const token = tokens[index];
    assert.ok(
      token.start + token.length <= state.source.line(token.line).length,
    );
    const previous = tokens[index - 1];
    if (previous)
      assert.ok(
        token.line > previous.line ||
          token.start >= previous.start + previous.length,
      );
  }
});

test("highlights use UTF-16 columns after supplementary characters and across CRLF", () => {
  const text =
    'fun 雪(𐐀x: String) {\r\n    val text = "😀"; println(𐐀x)\r\n}\r\n';
  const state = document(text);
  const tokens = tokensFor(state);
  const parameter = tokens.find(
    (token) => tokenTypes[token.type] === "parameter",
  );
  assert.equal(parameter?.length, 3);
  assert.equal(typeAt(state, text.lastIndexOf("println")), "function");
  assert.equal(typeAt(state, text.lastIndexOf("𐐀x")), "variable");
});

test("folds blocks, argument lists, raw strings, consecutive imports and line comments", () => {
  const state = document(
    [
      "import demo.One",
      "import demo.Two",
      "",
      "// first",
      "// second",
      "fun main() {",
      "    call(",
      "        1,",
      "        2",
      "    )",
      '    val text = """first',
      "second",
      '"""',
      "}",
    ].join("\n"),
  );
  const ranges = folds(
    state.source,
    syntax.folds.captures(state.tree.rootNode),
  );
  assert.ok(
    ranges.some(
      (fold) => fold.start === 0 && fold.end === 1 && fold.kind === "imports",
    ),
  );
  assert.ok(
    ranges.some(
      (fold) => fold.start === 3 && fold.end === 4 && fold.kind === "comment",
    ),
  );
  assert.ok(ranges.some((fold) => fold.start === 5 && fold.end === 12));
  assert.ok(ranges.some((fold) => fold.start === 6 && fold.end === 8));
  assert.ok(ranges.some((fold) => fold.start === 10 && fold.end === 11));
  assert.ok(ranges.every((fold) => fold.start < fold.end));
  assert.equal(
    new Set(ranges.map((fold) => `${fold.start}:${fold.end}`)).size,
    ranges.length,
  );
});

test("indentation preserves raw string contents and only changes leading whitespace", () => {
  const state = document(
    [
      "fun main() {",
      'val text = """',
      " raw text",
      '"""',
      "if (ready()) {",
      "call(",
      "1,",
      "2",
      ")",
      "}",
      "}",
    ].join("\n"),
  );
  const result = applyIndentEdits(state.source, reindent(state));
  assert.equal(
    result,
    [
      "fun main() {",
      '    val text = """',
      " raw text",
      '"""',
      "    if (ready()) {",
      "        call(",
      "            1,",
      "            2",
      "        )",
      "    }",
      "}",
    ].join("\n"),
  );
  const formatted = document(result);
  assert.deepEqual(reindent(formatted), [], "reindentation must be idempotent");
});

test("indentation handles unbraced control flow, tabs and selection boundaries", () => {
  const state = document(
    [
      "fun main() {",
      "if (ready())",
      "call()",
      "else",
      "finish()",
      "for (item in items)",
      "consume(item)",
      "}",
    ].join("\n"),
  );
  assert.equal(
    applyIndentEdits(state.source, reindent(state)),
    [
      "fun main() {",
      "    if (ready())",
      "        call()",
      "    else",
      "        finish()",
      "    for (item in items)",
      "        consume(item)",
      "}",
    ].join("\n"),
  );
  const selection = reindent(state, 1, 2, false);
  assert.deepEqual(
    selection.map((edit) => edit.line),
    [1, 2],
  );
  assert.deepEqual(
    selection.map((edit) => edit.text),
    ["\t", "\t\t"],
  );
});

test("indentation protects block comments and raw text inside interpolated strings", () => {
  const state = document(
    [
      "fun main() {",
      "/* comment",
      "  content stays",
      " */",
      'val text = f"""value ${',
      "call(",
      "1",
      ")",
      "}",
      " raw text stays",
      '"""',
      "}",
    ].join("\n"),
  );
  const edits = reindent(state);
  for (const line of [2, 3, 9, 10])
    assert.ok(!edits.some((edit) => edit.line === line));
  assert.ok(edits.some((edit) => edit.line === 5));
});

test("an unfinished block still indents a newly inserted line", () => {
  const state = syntax.document("test:incomplete", "fun main() {\n", 1);
  const edits = indentLines(
    state.source,
    syntax.indents.captures(state.tree.rootNode),
    { tabSize: 4, insertSpaces: true },
    1,
    1,
    true,
  );
  assert.deepEqual(edits, [{ line: 1, length: 0, text: "    " }]);
});

test("multiline inheritance and property accessors have their own indentation", () => {
  const state = document(
    [
      "class Value :",
      "Hash,",
      "Printable {",
      "val code: Int",
      "get() {",
      "return 1",
      "}",
      "}",
    ].join("\n"),
  );
  assert.equal(
    applyIndentEdits(state.source, reindent(state)),
    [
      "class Value :",
      "    Hash,",
      "    Printable {",
      "    val code: Int",
      "        get() {",
      "            return 1",
      "        }",
      "}",
    ].join("\n"),
  );
});

test("incremental edits can change a plain string into an interpolated string", () => {
  const before = 'val text = "value ${雪}"\n';
  const state = document(before);
  const offset = before.indexOf('"');
  const inserted = `${before.slice(0, offset)}f${before.slice(offset)}`;
  state.update(
    [{ rangeOffset: offset, rangeLength: 0, text: "f" }],
    inserted,
    2,
  );
  assert.equal(typeAt(state, inserted.indexOf("雪")), "variable");
  state.update([{ rangeOffset: offset, rangeLength: 1, text: "" }], before, 3);
  assert.equal(typeAt(state, before.indexOf("雪")), "string");
});

test("multiline where constraints and expression bodies continue the declaration", () => {
  const state = document(
    ["fun <T> echo(value: T): T", "where T: ref,", "T: Hash =", "value"].join(
      "\n",
    ),
  );
  assert.equal(
    applyIndentEdits(state.source, reindent(state)),
    [
      "fun <T> echo(value: T): T",
      "    where T: ref,",
      "        T: Hash =",
      "    value",
    ].join("\n"),
  );
});

test("incremental multi-edit updates match a fresh parse after Unicode and newline edits", () => {
  const before =
    'fun 雪() {\r\n    val text = "😀"\r\n    println(text)\r\n}\r\n';
  const state = document(before);
  const changes: Edit[] = [
    { rangeOffset: before.indexOf("😀"), rangeLength: 2, text: "雪😀" },
    {
      rangeOffset: before.indexOf("println"),
      rangeLength: "println".length,
      text: "consume",
    },
    {
      rangeOffset: before.indexOf("    val"),
      rangeLength: 0,
      text: "    // inserted\r\n",
    },
  ];
  let afterText = before;
  for (const edit of [...changes].sort(
    (a, b) => b.rangeOffset - a.rangeOffset,
  )) {
    afterText =
      afterText.slice(0, edit.rangeOffset) +
      edit.text +
      afterText.slice(edit.rangeOffset + edit.rangeLength);
  }
  state.update(changes, afterText, 2);
  const freshParser = new Parser().setLanguage(syntax.language);
  const fresh = freshParser.parse(afterText);
  assert.ok(fresh);
  try {
    assert.equal(state.tree.rootNode.hasError, false);
    assert.equal(state.tree.rootNode.toString(), fresh.rootNode.toString());
    assert.equal(state.tree.rootNode.endIndex, fresh.rootNode.endIndex);
    assert.deepEqual(
      state.tree.rootNode.endPosition,
      fresh.rootNode.endPosition,
    );
    assert.equal(typeAt(state, afterText.indexOf("consume")), "function");
  } finally {
    fresh.delete();
    freshParser.delete();
  }
});

test("CR and LF coordinates follow each API's line convention", () => {
  const source = new SourceText("a\rb\r\nc\n😀x");
  assert.deepEqual(source.positionAt(2), { row: 1, column: 0 });
  assert.deepEqual(source.treePositionAt(2), { row: 0, column: 2 });
  assert.deepEqual(source.positionAt(9), { row: 3, column: 2 });
});

test("closing a document frees its state and reopening it reads the new text", () => {
  const first = syntax.document("test:reopen", "fun first() {}", 1);
  assert.equal(first.tree.rootNode.hasError, false);
  syntax.close("test:reopen");
  const second = syntax.document("test:reopen", "fun second() {}", 1);
  assert.notEqual(first, second);
  assert.equal(
    second.tree.rootNode.namedChild(0)?.childForFieldName("name")?.text,
    "second",
  );
});
