const { commaSep, commaSep1 } = require("./helpers");

const PREC = {
  OR: 1,
  AND: 2,
  EQUALITY: 3,
  COMPARISON: 4,
  MEMBERSHIP: 5,
  ELVIS: 6,
  INFIX: 7,
  RANGE: 8,
  ADD: 9,
  MULTIPLY: 10,
  CAST: 11,
  UNARY: 12,
  POSTFIX: 13,
};

module.exports = {
  block: ($) =>
    seq("{", repeat(choice(";", seq($._statement, $._terminator))), "}"),
  _statement: ($) =>
    choice(
      $._declaration,
      $._expression,
      $.assignment,
      $.while_statement,
      $.do_while_statement,
      $.for_statement,
      $.block,
      $.safety_block,
    ),
  _control_body: ($) => $._statement,
  safety_block: ($) => seq($.annotation, $.block),

  _expression: ($) =>
    choice(
      $.identifier,
      $._literal,
      $.interpolated_string,
      $.raw_interpolated_string,
      $.unit_literal,
      $.parenthesized_expression,
      $.tuple_literal,
      $.array_literal,
      $.unary_expression,
      $.binary_expression,
      $.infix_expression,
      $.type_expression,
      $.call_expression,
      $.navigation_expression,
      $.index_expression,
      $.postfix_expression,
      $.copy_expression,
      $.callable_reference,
      $.this_expression,
      $.super_expression,
      $.lambda_literal,
      $.anonymous_function,
      $.object_literal,
      $.labeled_expression,
      $.if_expression,
      $.when_expression,
      $.try_expression,
      $.context_expression,
      $.return_expression,
      $.throw_expression,
      $.jump_expression,
    ),
  unit_literal: (_) => seq("(", ")"),
  parenthesized_expression: ($) => seq("(", $._expression, ")"),
  tuple_literal: ($) =>
    seq("(", $._expression, ",", commaSep($._expression), ")"),
  array_literal: ($) => seq("[", commaSep($._expression), "]"),

  unary_expression: ($) =>
    prec(
      PREC.UNARY,
      seq(
        field("operator", choice("+", "-", "!", "++", "--")),
        field("operand", $._expression),
      ),
    ),
  binary_expression: ($) =>
    choice(
      ...[
        [PREC.OR, "||"],
        [PREC.AND, "&&"],
        [PREC.EQUALITY, choice("==", "!=", "===", "!==")],
        [PREC.COMPARISON, choice("<", "<=", ">", ">=")],
        [PREC.MEMBERSHIP, choice("in", "!in")],
        [PREC.ELVIS, "?:"],
        [PREC.RANGE, choice("..", "..<")],
        [PREC.ADD, choice("+", "-")],
        [PREC.MULTIPLY, choice("*", "/", "%")],
      ].map(([precedence, operator]) =>
        (precedence === PREC.ELVIS ? prec.right : prec.left)(
          precedence,
          seq(
            field("left", $._expression),
            field("operator", operator),
            field("right", $._expression),
          ),
        ),
      ),
    ),
  infix_expression: ($) =>
    prec.left(
      PREC.INFIX,
      seq(
        field("left", $._expression),
        field("operator", $.identifier),
        field("right", $._expression),
      ),
    ),
  type_expression: ($) =>
    choice(
      ...[
        [PREC.MEMBERSHIP, choice("is", "!is")],
        [PREC.CAST, choice("as", "as?")],
      ].map(([precedence, operator]) =>
        prec.left(
          precedence,
          seq(
            field("value", $._expression),
            field("operator", operator),
            field("type", $._type),
          ),
        ),
      ),
    ),
  assignment: ($) =>
    prec.right(
      1,
      seq(
        field("left", $._expression),
        field("operator", choice("=", "+=", "-=", "*=", "/=", "%=")),
        field("right", $._expression),
      ),
    ),

  call_expression: ($) =>
    prec(
      PREC.POSTFIX,
      seq(
        field("function", $._expression),
        optional($.type_arguments),
        choice(
          $.arguments,
          prec.dynamic(1, seq($.arguments, $.trailing_lambda)),
          $.trailing_lambda,
        ),
      ),
    ),
  arguments: ($) => seq("(", commaSep($.argument), ")"),
  argument: ($) =>
    seq(
      optional(seq(field("name", $.identifier), "=")),
      optional("*"),
      field("value", $._expression),
    ),
  trailing_lambda: ($) => seq(optional($.label), $.lambda_literal),
  navigation_expression: ($) =>
    prec.left(
      PREC.POSTFIX,
      seq(
        field("object", choice($._expression, $.user_type)),
        choice(".", "?."),
        field("member", $.identifier),
      ),
    ),
  index_expression: ($) =>
    prec.left(
      PREC.POSTFIX,
      seq(field("object", $._expression), "[", commaSep1($._expression), "]"),
    ),
  postfix_expression: ($) =>
    prec.left(PREC.POSTFIX, seq($._expression, choice("!!", "++", "--"))),
  copy_expression: ($) =>
    prec.left(
      PREC.POSTFIX,
      seq(field("base", $._expression), ".", $.field_updates),
    ),
  field_updates: ($) => seq("{", commaSep1($.field_update), "}"),
  field_update: ($) =>
    seq(field("name", $.identifier), ":", field("value", $._expression)),
  callable_reference: ($) =>
    prec.left(
      PREC.POSTFIX,
      seq(
        optional(field("receiver", choice($._expression, $.user_type))),
        "::",
        field("name", $.identifier),
      ),
    ),
  this_expression: ($) =>
    prec.right(
      PREC.POSTFIX,
      seq("this", optional(seq("@", field("label", $.identifier)))),
    ),
  super_expression: ($) =>
    prec.right(
      PREC.POSTFIX,
      seq(
        "super",
        optional(seq("<", $._type, ">")),
        optional(seq("@", field("label", $.identifier))),
      ),
    ),

  lambda_literal: ($) =>
    seq(
      optional("suspend"),
      "{",
      optional($.lambda_parameters),
      repeat(choice(";", seq($._statement, $._terminator))),
      "}",
    ),
  lambda_parameters: ($) => seq(commaSep1($.lambda_parameter), "->"),
  lambda_parameter: ($) =>
    seq(
      field("name", $._binding_pattern),
      optional(seq(":", field("type", $._type))),
    ),
  anonymous_function: ($) =>
    prec.right(
      seq(
        optional("suspend"),
        "fun",
        optional($._receiver),
        $.parameters,
        optional(seq(":", field("return_type", $._type))),
        $._function_body,
      ),
    ),
  object_literal: ($) =>
    prec(1, seq("object", optional($.delegation_specifiers), $.class_body)),
  label: ($) => prec(1, seq($.identifier, "@")),
  labeled_expression: ($) => prec.right(seq($.label, $._expression)),

  if_expression: ($) =>
    choice(
      $._if_head,
      prec.right(
        1,
        seq($._if_head, "else", field("alternative", $._control_body)),
      ),
      seq($._if_head, ";", "else", field("alternative", $._control_body)),
    ),
  _if_head: ($) =>
    seq(
      "if",
      "(",
      field("condition", $._expression),
      ")",
      field("consequence", $._control_body),
    ),
  while_statement: ($) =>
    seq(
      optional($.label),
      "while",
      "(",
      field("condition", $._expression),
      ")",
      field("body", $._control_body),
    ),
  do_while_statement: ($) =>
    seq(
      optional($.label),
      "do",
      field("body", $._control_body),
      optional(";"),
      "while",
      "(",
      field("condition", $._expression),
      ")",
    ),
  for_statement: ($) =>
    seq(
      optional($.label),
      "for",
      "(",
      field("pattern", $._binding_pattern),
      "in",
      field("value", $._expression),
      ")",
      field("body", $._control_body),
    ),
  return_expression: ($) =>
    prec.right(
      seq(
        "return",
        optional(seq("@", field("label", $.identifier))),
        optional(field("value", $._expression)),
      ),
    ),
  throw_expression: ($) =>
    prec.right(1, seq("throw", field("value", $._expression))),
  jump_expression: ($) =>
    prec.right(
      seq(
        choice("break", "continue"),
        optional(seq("@", field("label", $.identifier))),
      ),
    ),

  when_expression: ($) =>
    seq(
      "when",
      optional(seq("(", choice($.when_subject, $._expression), ")")),
      $.when_body,
    ),
  when_subject: ($) =>
    seq(
      "val",
      field("name", $._binding_pattern),
      optional(seq(":", $._type)),
      "=",
      field("value", $._expression),
    ),
  when_body: ($) => seq("{", repeat($.when_entry), "}"),
  when_entry: ($) =>
    prec(
      1,
      seq(
        choice(
          seq("case", field("pattern", $._pattern), optional($.when_guard)),
          seq("else", optional($.when_guard)),
          seq(
            $.when_condition,
            optional(
              choice($.when_guard, seq(",", commaSep($.when_condition))),
            ),
          ),
        ),
        "->",
        field("body", $._control_body),
        $._arm_terminator,
      ),
    ),
  when_condition: ($) =>
    choice(
      $._expression,
      seq(choice("is", "!is"), $._type),
      seq(choice("in", "!in"), $._expression),
    ),
  when_guard: ($) => seq("if", $._expression),
  try_expression: ($) =>
    prec.right(
      seq(
        "try",
        $.block,
        choice(
          seq(repeat1($.catch_clause), optional($.finally_clause)),
          $.finally_clause,
        ),
      ),
    ),
  catch_clause: ($) =>
    prec(PREC.POSTFIX, seq("catch", "(", $.parameter, ")", $.block)),
  finally_clause: ($) => prec(PREC.POSTFIX, seq("finally", $.block)),
  context_expression: ($) =>
    prec.dynamic(
      2,
      seq("context", "(", field("value", $._expression), ")", $.block),
    ),
};
