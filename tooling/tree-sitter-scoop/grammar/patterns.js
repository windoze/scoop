const { commaSep } = require("./helpers");

module.exports = {
  _binding_pattern: ($) =>
    choice($.identifier, $.tuple_pattern, $.struct_pattern),
  _pattern: ($) =>
    choice(
      $._binding_pattern,
      $.variant_pattern,
      $._literal,
      $.negative_literal_pattern,
    ),
  rest_pattern: (_) => "..",
  tuple_pattern: ($) =>
    seq("(", commaSep(choice($._pattern, $.rest_pattern)), ")"),
  struct_pattern: ($) =>
    seq(optional(field("type", $.qualified_identifier)), $.field_patterns),
  field_patterns: ($) =>
    seq("{", commaSep(choice($.field_pattern, $.rest_pattern)), "}"),
  field_pattern: ($) =>
    seq(
      field("name", $.identifier),
      optional(seq(":", field("pattern", $._pattern))),
    ),
  variant_pattern: ($) =>
    choice(
      seq(field("name", $.qualified_identifier), $.tuple_pattern),
      seq($.identifier, ".", field("name", $.qualified_identifier)),
    ),
  negative_literal_pattern: ($) =>
    seq("-", choice($.integer_literal, $.float_literal)),
};
