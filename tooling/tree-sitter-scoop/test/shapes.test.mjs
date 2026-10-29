import assert from "node:assert/strict";
import { after, test } from "node:test";
import { errors, parser, query } from "./helpers.mjs";

after(() => parser.delete());

function check(source, inspect) {
  const tree = parser.parse(source);
  try {
    assert.equal(tree.rootNode.hasError, false, errors(tree).join("\n"));
    inspect(tree.rootNode);
  } finally {
    tree.delete();
  }
}

test("all catch clauses remain part of the same try expression", () => {
  check(
    "fun f() { try { run() } catch (error: First) {} catch (_: Second) {} catch (error: Last) { throw error } finally { finish() } }",
    (root) => {
      const expression = root.descendantsOfType("try_expression");
      assert.equal(expression.length, 1);
      assert.equal(expression[0].descendantsOfType("catch_clause").length, 3);
      assert.equal(expression[0].descendantsOfType("finally_clause").length, 1);
      assert.equal(root.descendantsOfType("infix_expression").length, 0);
    },
  );
});

test("an else binds to the nearest if and a separator can precede else", () => {
  check(
    "fun f() { if (a) if (b) first() else second(); if (c) third(); /* boundary */ else fourth() }",
    (root) => {
      const branches = root.descendantsOfType("if_expression");
      assert.equal(branches.length, 3);
      assert.equal(branches[0].childForFieldName("alternative"), null);
      assert.equal(
        branches[1].childForFieldName("alternative").text,
        "second()",
      );
      assert.equal(
        branches[2].childForFieldName("alternative").text,
        "fourth()",
      );
    },
  );
});

test("when guards and conditional bodies keep their own else and arrow", () => {
  check(
    [
      "fun f() = when (x) {",
      "    0 -> if (ready()) first()",
      '         else if (alsoReady("->")) second() else third()',
      '    else /* boundary */ if (x > 0 && text != "->") -> fourth()',
      "    else if (x < 0 /* ) -> */)",
      "        -> fifth()",
      "    else -> last()",
      "}",
    ].join("\n"),
    (root) => {
      const arms = root.descendantsOfType("when_entry");
      assert.equal(arms.length, 4);
      assert.equal(arms[0].childForFieldName("body").type, "if_expression");
      assert.equal(arms[0].descendantsOfType("if_expression").length, 2);
      assert.equal(root.descendantsOfType("when_guard").length, 2);
    },
  );
});

test("trailing lambdas belong to the call's argument list", () => {
  check(
    "fun f() { service.run(1) { work() }; consume((read() xor 1)) }",
    (root) => {
      const call = root
        .descendantsOfType("call_expression")
        .find((node) =>
          node.namedChildren.some((child) => child.type === "trailing_lambda"),
        );
      assert.ok(call);
      assert.equal(
        call.childForFieldName("function").type,
        "navigation_expression",
      );
      assert.ok(call.namedChildren.some((child) => child.type === "arguments"));
      assert.equal(root.descendantsOfType("infix_expression").length, 1);
    },
  );
});

test("context scopes use blocks while qualified and incomplete forms are calls", () => {
  check(
    "fun f() { context(log) { run() }; service.context(log) { run() }; context(log) }",
    (root) => {
      const scopes = root.descendantsOfType("context_expression");
      assert.equal(scopes.length, 1);
      assert.equal(scopes[0].lastNamedChild.type, "block");
      assert.equal(root.descendantsOfType("lambda_literal").length, 1);
      assert.ok(
        root
          .descendantsOfType("call_expression")
          .some(
            (node) => node.childForFieldName("function").text === "context",
          ),
      );
    },
  );
});

test("comment delimiters inside strings remain text", () => {
  check(
    'val a = "// text"\nval b = f"/* text */ ${1}"\nval c = """// raw"""',
    (root) => {
      assert.equal(
        root.descendantsOfType(["line_comment", "block_comment"]).length,
        0,
      );
      assert.equal(root.descendantsOfType("interpolation").length, 1);
    },
  );
});

test("a prefix operator on the next line starts a new statement", () => {
  check("fun f() { run()\n!ready()\n}", (root) => {
    const body = root.descendantsOfType("block")[0];
    assert.deepEqual(
      body.namedChildren.map((node) => node.type),
      ["call_expression", "unary_expression"],
    );
  });
});

test("conditional conformances remain members of their generic type", () => {
  check(
    "struct Box<T> { override interface Equals<Box<T>> where T: Equals<T> { override fun equals(other: Box<T>): Boolean = true } }",
    (root) => {
      const members = root.descendantsOfType("conditional_conformance");
      assert.equal(members.length, 1);
      assert.equal(members[0].childForFieldName("type").text, "Equals<Box<T>>");
      assert.equal(members[0].descendantsOfType("where_clause").length, 1);
    },
  );
});

test("get and set identifiers begin new statements after a newline", () => {
  check(
    "fun f() {\nval get = source\nval set = target\nget.clear()\nset.clear()\n}",
    (root) => {
      assert.equal(root.descendantsOfType("property_declaration").length, 2);
      assert.equal(root.descendantsOfType("call_expression").length, 2);
    },
  );
});

test("a lambda can spell its empty parameter list", () => {
  check("val f: () -> Int = { -> 42 }", (root) => {
    const parameters = root.descendantsOfType("lambda_parameters");
    assert.equal(parameters.length, 1);
    assert.equal(parameters[0].namedChildCount, 0);
  });
});

test("receiver function types in generic calls keep their callable highlight", async () => {
  const source = "val info = typeInfo<String.(Int, Long) -> Boolean>().type";
  const highlights = await query("highlights");
  try {
    check(source, (root) => {
      const types = root.descendantsOfType("function_type");
      assert.equal(types.length, 1);
      assert.equal(types[0].childForFieldName("receiver").text, "String");
      assert.equal(root.descendantsOfType("call_expression").length, 1);
      assert.ok(
        highlights
          .captures(root)
          .some(
            ({ name, node }) =>
              name === "function.call" && node.text === "typeInfo",
          ),
      );
    });
  } finally {
    highlights.delete();
  }
});
