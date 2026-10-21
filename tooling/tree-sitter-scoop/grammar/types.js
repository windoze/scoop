const { commaSep, commaSep1 } = require("./helpers");

module.exports = {
  _type: ($) => choice($._unannotated_type, $.annotated_type),
  _unannotated_type: ($) =>
    choice(
      $.user_type,
      $.nullable_type,
      $.tuple_type,
      $.parenthesized_type,
      $.function_type,
    ),
  annotated_type: ($) =>
    prec.right(seq(repeat1($.annotation), $._unannotated_type)),
  user_type: ($) =>
    prec.dynamic(
      -1,
      seq(
        $._type_identifier,
        optional($.type_arguments),
        repeat(seq(".", $._type_identifier, optional($.type_arguments))),
      ),
    ),
  _type_identifier: ($) => alias($.identifier, $.type_identifier),
  type_arguments: ($) => seq("<", commaSep1($._type), ">"),
  nullable_type: ($) => prec.left(15, seq($._type, "?")),
  parenthesized_type: ($) => seq("(", $._type, ")"),
  tuple_type: ($) =>
    seq("(", optional(seq($._type, ",", commaSep($._type))), ")"),
  function_type: ($) =>
    prec.right(
      1,
      seq(
        optional("suspend"),
        optional(seq(field("receiver", $._type), choice(".", "?."))),
        $.function_type_parameters,
        "->",
        field("return_type", $._type),
      ),
    ),
  function_type_parameters: ($) => seq("(", commaSep($._type), ")"),
  type_parameters: ($) => seq("<", commaSep1($.type_parameter), ">"),
  type_parameter: ($) =>
    seq(
      repeat(choice($.annotation, "reified")),
      field("name", $.identifier),
      optional(seq(":", $._type_bound)),
    ),
  _type_bound: ($) => choice($._type, $.negative_type_bound),
  negative_type_bound: ($) => seq("!", $._type),
  where_clause: ($) => {
    const constraint = choice($.type_constraint, $.member_requirements);
    return prec.right(seq("where", constraint, repeat(seq(",", constraint))));
  },
  type_constraint: ($) =>
    seq(field("subject", $.qualified_identifier), ":", $._type_bound),
  member_requirements: ($) =>
    seq("requires", optional($.requirement_parameters), $.requirements_body),
  requirement_parameters: ($) => seq("(", commaSep($.context_parameter), ")"),
  requirements_body: ($) =>
    seq("{", repeat1(seq($.required_call, $._terminator)), "}"),
  required_call: ($) =>
    seq(
      repeat(choice($.annotation, "suspend")),
      field("target", $.qualified_identifier),
      optional($.type_arguments),
      $.requirement_arguments,
      optional(seq(":", field("return_type", $._type))),
    ),
  requirement_arguments: ($) => seq("(", commaSep($.requirement_argument), ")"),
  requirement_argument: ($) =>
    seq(
      optional(seq(field("name", $.identifier), "=")),
      optional("*"),
      field("value", $.identifier),
      optional(seq(":", field("type", $._type))),
    ),
};
