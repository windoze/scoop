const declarations = require("./grammar/declarations");
const expressions = require("./grammar/expressions");
const types = require("./grammar/types");
const patterns = require("./grammar/patterns");
const literals = require("./grammar/literals");

module.exports = grammar({
  name: "scoop",

  extras: ($) => [/\s/, $.line_comment, $.block_comment],
  word: ($) => $._identifier,
  supertypes: ($) => [
    $._expression,
    $._type,
    $._declaration,
    $._statement,
    $._pattern,
    $._binding_pattern,
    $._literal,
  ],
  inline: ($) => [$._type_identifier],
  externals: ($) => [
    $._automatic_semicolon,
    $._arm_semicolon,
    $.raw_string_content,
    $.raw_fstring_content,
    $.string_content,
    $.fstring_content,
  ],

  conflicts: ($) => [
    [$.qualified_identifier],
    [$.user_type],
    [$.property_declaration],
    [$.call_expression],
    [$.if_expression],
    [$.identifier, $.modifier],
    [$.identifier, $.context_expression],
    [$.identifier, $.try_expression],
    [$.identifier, $.context_parameters],
    [$.identifier, $.context_parameters, $.context_expression],
    [$.qualified_identifier, $.user_type],
    [$.qualified_identifier, $.user_type, $._binding_pattern],
    [$.qualified_identifier, $._binding_pattern],
    [$._type, $.annotated_type],
    [$.annotated_type],
    [$.parameter, $.annotated_type],
    [$.class_parameter, $.annotated_type],
    [$.user_type, $._binding_pattern],
    [$._binding_pattern, $._expression],
    [$._pattern, $._expression],
    [$._binding_pattern, $.field_pattern],
    [$.unit_literal, $.tuple_pattern],
    [$.tuple_type, $.tuple_pattern],
    [$.tuple_type, $.unit_literal],
    [$.tuple_type, $.function_type_parameters],
    [$.parenthesized_type, $.function_type_parameters],
    [$.function_type_parameters, $.unit_literal],
    [$.function_type_parameters, $.tuple_pattern],
    [$.tuple_type],
    [$.modifiers, $.enum_variant],
    [$.qualified_identifier, $.user_type, $._expression],
    [$.qualified_identifier, $.user_type, $.variant_pattern],
    [$.qualified_identifier, $.user_type, $.variant_pattern, $._expression],
    [$.modifier, $.anonymous_function],
    [$.unary_expression, $.infix_expression, $.call_expression],
    [$.binary_expression, $.infix_expression, $.call_expression],
    [$.unary_expression, $.binary_expression, $.call_expression],
    [$.user_type, $._expression],
    [$.lambda_literal, $.block],
    [$.field_patterns, $.lambda_literal],
    [$.negative_literal_pattern, $._literal],
    [$.call_expression, $.binary_expression],
  ],

  rules: {
    source_file: ($) =>
      seq(
        optional(seq($.package_header, $._terminator)),
        repeat(seq($.import_header, $._terminator)),
        repeat(choice(";", seq($._declaration, $._terminator))),
      ),

    _terminator: ($) => choice(";", $._automatic_semicolon),
    _arm_terminator: ($) => choice(";", $._arm_semicolon),
    package_header: ($) => seq("package", $.qualified_identifier),
    import_header: ($) =>
      seq(
        optional("public"),
        "import",
        $.qualified_identifier,
        optional(
          choice(seq(".", "*"), seq("as", field("alias", $.identifier))),
        ),
      ),
    qualified_identifier: ($) =>
      seq($.identifier, repeat(seq(".", $.identifier))),

    identifier: ($) =>
      choice(
        $._identifier,
        alias(
          choice(
            "open",
            "final",
            "abstract",
            "override",
            "super",
            "object",
            "sealed",
            "companion",
            "init",
            "constructor",
            "try",
            "catch",
            "finally",
            "throw",
            "public",
            "internal",
            "private",
            "protected",
            "typealias",
            "context",
            "operator",
            "get",
            "set",
            "by",
            "where",
            "value",
            "ref",
            "inner",
            "const",
            "lateinit",
            "inline",
            "tailrec",
            "crossinline",
            "noinline",
            "reified",
            "annotation",
            "case",
            "requires",
          ),
          $._identifier,
        ),
      ),

    _identifier: (_) =>
      token(
        choice(
          /[\p{XID_Start}_][\p{XID_Continue}]*/u,
          // biome-ignore lint/suspicious/noControlCharactersInRegex: Escaped identifiers exclude control characters.
          /`[^`\x00-\x1f\x7f-\x9f]+`/,
        ),
      ),

    line_comment: (_) => token(seq("//", /[^\r\n]*/)),
    block_comment: (_) => token(seq("/*", /[^*]*\*+([^/*][^*]*\*+)*/, "/")),

    ...declarations,
    ...types,
    ...patterns,
    ...expressions,
    ...literals,
  },
});
