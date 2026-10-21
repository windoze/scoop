const { commaSep, commaSep1 } = require("./helpers");

module.exports = {
  _declaration: ($) =>
    choice(
      $.function_declaration,
      $.property_declaration,
      $.class_declaration,
      $.struct_declaration,
      $.interface_declaration,
      $.enum_declaration,
      $.object_declaration,
      $.companion_object,
      $.annotation_class_declaration,
      $.type_alias,
    ),

  modifiers: ($) => repeat1(choice($.annotation, $.modifier)),
  modifier: (_) =>
    choice(
      "public",
      "internal",
      "private",
      "protected",
      "open",
      "final",
      "abstract",
      "sealed",
      "override",
      "inner",
      "const",
      "lateinit",
      "suspend",
      "operator",
      "infix",
      "inline",
      "tailrec",
    ),

  annotation: ($) =>
    prec.right(
      seq(
        "@",
        optional(seq(field("target", $.identifier), ":")),
        field("name", $.qualified_identifier),
        optional($.arguments),
      ),
    ),

  context_parameters: ($) =>
    seq("context", "(", commaSep1($.context_parameter), ")"),
  context_parameter: ($) =>
    seq(field("name", $.identifier), ":", field("type", $._type)),

  function_declaration: ($) =>
    prec.right(
      seq(
        optional($.context_parameters),
        optional($.modifiers),
        "fun",
        optional($.type_parameters),
        optional($._receiver),
        field("name", $.identifier),
        $.parameters,
        optional(seq(":", field("return_type", $._type))),
        optional($.where_clause),
        optional($._function_body),
      ),
    ),
  _receiver: ($) => prec(2, seq(field("receiver", $._type), choice(".", "?."))),
  parameters: ($) => seq("(", commaSep($.parameter), ")"),
  parameter: ($) =>
    seq(
      repeat(choice($.annotation, "vararg", "crossinline", "noinline")),
      field("name", $.identifier),
      ":",
      field("type", $._type),
      optional(seq("=", field("default_value", $._expression))),
    ),
  _function_body: ($) => choice(field("body", $.block), $.expression_body),
  expression_body: ($) => seq("=", field("body", $._expression)),

  property_declaration: ($) =>
    prec.dynamic(
      1,
      seq(
        optional($.context_parameters),
        optional($.modifiers),
        choice("val", "var"),
        optional($.type_parameters),
        optional($._receiver),
        field("name", $._binding_pattern),
        optional(seq(":", field("type", $._type))),
        optional($.where_clause),
        optional(
          choice(
            seq("=", field("value", $._expression)),
            seq("by", field("delegate", $._expression)),
          ),
        ),
        repeat(seq(optional($._automatic_semicolon), $.accessor)),
      ),
    ),
  accessor: ($) =>
    prec.right(
      1,
      seq(
        optional($.modifiers),
        choice(
          seq("get", "(", ")"),
          seq(
            "set",
            "(",
            optional(
              seq(
                repeat(choice("crossinline", "noinline")),
                field("parameter", $.identifier),
              ),
            ),
            ")",
          ),
        ),
        optional($._function_body),
      ),
    ),

  class_declaration: ($) =>
    prec.right(
      seq(
        optional($.modifiers),
        "class",
        field("name", $.identifier),
        optional($.type_parameters),
        optional($.primary_constructor),
        optional($.delegation_specifiers),
        optional($.where_clause),
        optional($.class_body),
      ),
    ),
  struct_declaration: ($) =>
    prec.right(
      seq(
        optional($.modifiers),
        "struct",
        field("name", $.identifier),
        optional($.type_parameters),
        optional($.primary_constructor),
        optional($.delegation_specifiers),
        optional($.where_clause),
        optional($.class_body),
      ),
    ),
  interface_declaration: ($) =>
    prec.right(
      seq(
        optional($.modifiers),
        "interface",
        field("name", $.identifier),
        optional($.type_parameters),
        optional($.delegation_specifiers),
        optional($.where_clause),
        optional($.class_body),
      ),
    ),
  object_declaration: ($) =>
    prec.right(
      1,
      seq(
        optional($.modifiers),
        "object",
        field("name", $.identifier),
        optional($.delegation_specifiers),
        optional($.class_body),
      ),
    ),
  companion_object: ($) =>
    prec.right(
      1,
      seq(
        optional($.modifiers),
        "companion",
        "object",
        optional(field("name", $.identifier)),
        optional($.delegation_specifiers),
        optional($.class_body),
      ),
    ),
  annotation_class_declaration: ($) =>
    prec.right(
      1,
      seq(
        optional($.modifiers),
        "annotation",
        "class",
        field("name", $.identifier),
        optional($.primary_constructor),
      ),
    ),
  primary_constructor: ($) =>
    seq(
      optional(seq(optional($.modifiers), "constructor")),
      $.class_parameters,
    ),
  class_parameters: ($) => seq("(", commaSep($.class_parameter), ")"),
  class_parameter: ($) =>
    seq(
      repeat(
        choice(
          $.annotation,
          "public",
          "internal",
          "private",
          "protected",
          "override",
          "vararg",
          "crossinline",
          "noinline",
        ),
      ),
      optional(choice("val", "var")),
      field("name", $.identifier),
      ":",
      field("type", $._type),
      optional(seq("=", field("default_value", $._expression))),
    ),
  delegation_specifiers: ($) =>
    prec.right(
      seq(
        ":",
        $.delegation_specifier,
        repeat(seq(",", $.delegation_specifier)),
      ),
    ),
  delegation_specifier: ($) =>
    prec.right(
      seq(
        field("type", $._type),
        optional(
          choice($.arguments, seq("by", field("delegate", $._expression))),
        ),
      ),
    ),
  class_body: ($) =>
    seq("{", repeat(choice(";", seq($._member, $._terminator))), "}"),
  _member: ($) =>
    choice(
      $._declaration,
      $.secondary_constructor,
      $.init_block,
      $.release_block,
    ),
  secondary_constructor: ($) =>
    prec.right(
      1,
      seq(
        optional($.modifiers),
        "constructor",
        $.parameters,
        optional(seq(":", choice("this", "super"), $.arguments)),
        optional($.block),
      ),
    ),
  init_block: ($) => prec(1, seq("init", $.block)),
  release_block: ($) => prec(1, seq("release", $.block)),

  enum_declaration: ($) =>
    seq(
      optional($.modifiers),
      "enum",
      field("name", $.identifier),
      optional($.type_parameters),
      optional($.delegation_specifiers),
      optional($.where_clause),
      $.enum_body,
    ),
  enum_body: ($) =>
    seq(
      "{",
      repeat(seq($.enum_variant, choice(",", $._automatic_semicolon))),
      repeat(choice(";", seq($._declaration, $._terminator))),
      "}",
    ),
  enum_variant: ($) =>
    prec.right(
      seq(
        repeat($.annotation),
        field("name", $.identifier),
        optional(
          choice(
            $.enum_payload,
            $.enum_fields,
            seq("=", field("value", $._expression)),
          ),
        ),
      ),
    ),
  enum_payload: ($) =>
    seq("(", commaSep(choice($.class_parameter, $._type)), ")"),
  enum_fields: ($) => seq("{", commaSep($.enum_field), "}"),
  enum_field: ($) =>
    seq(
      repeat($.annotation),
      field("name", $.identifier),
      ":",
      field("type", $._type),
    ),

  type_alias: ($) =>
    prec.right(
      1,
      seq(
        optional($.modifiers),
        "typealias",
        field("name", $.identifier),
        optional($.type_parameters),
        optional($.where_clause),
        "=",
        field("type", $._type),
      ),
    ),
};
