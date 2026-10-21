module.exports = {
  _literal: ($) =>
    choice(
      $.integer_literal,
      $.float_literal,
      $.boolean_literal,
      $.character_literal,
      $.string_literal,
      $.raw_string_literal,
    ),
  integer_literal: (_) =>
    token(
      seq(
        choice(
          /0[xX][\da-fA-F](_?[\da-fA-F])*/,
          /0[bB][01](_?[01])*/,
          /\d(_?\d)*/,
        ),
        optional(choice(/[uU][lL]?/, /[lL]/)),
      ),
    ),
  float_literal: (_) =>
    token(
      choice(
        /\d(_?\d)*\.\d(_?\d)*([eE][+-]?\d(_?\d)*)?[fF]?/,
        /\.\d(_?\d)*([eE][+-]?\d(_?\d)*)?[fF]?/,
        /\d(_?\d)*[eE][+-]?\d(_?\d)*[fF]?/,
        /\d(_?\d)*[fF]/,
      ),
    ),
  boolean_literal: (_) => choice("true", "false"),
  character_literal: ($) =>
    seq(
      "'",
      choice(token.immediate(/[^'\\\r\n]/), $.escape_sequence),
      token.immediate("'"),
    ),
  escape_sequence: (_) =>
    token.immediate(
      seq(
        "\\",
        choice(/[tbnr'"\\$]/, /u[\da-fA-F]{4}/, /u\{[\da-fA-F]{1,6}\}/),
      ),
    ),
  string_literal: ($) =>
    seq(
      '"',
      repeat(choice($.string_content, $.escape_sequence)),
      token.immediate('"'),
    ),
  raw_string_literal: ($) => seq('"""', repeat($.raw_string_content), '"""'),
  interpolated_string: ($) =>
    seq(
      'f"',
      repeat(choice($.fstring_content, $.escape_sequence, $.interpolation)),
      token.immediate('"'),
    ),
  raw_interpolated_string: ($) =>
    seq('f"""', repeat(choice($.raw_fstring_content, $.interpolation)), '"""'),
  interpolation: ($) => seq(token.immediate(prec(2, "${")), $._expression, "}"),
};
