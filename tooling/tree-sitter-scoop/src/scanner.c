#include "tree_sitter/parser.h"

#include <string.h>

enum TokenType {
  AUTOMATIC_SEMICOLON,
  ARM_SEMICOLON,
  RAW_STRING_CONTENT,
  RAW_FSTRING_CONTENT,
  STRING_CONTENT,
  FSTRING_CONTENT,
};

static bool scan_string_content(TSLexer *lexer, bool interpolated) {
  bool has_content = false;
  lexer->mark_end(lexer);
  while (!lexer->eof(lexer) && lexer->lookahead != '"' &&
         lexer->lookahead != '\\' && lexer->lookahead != '\n' &&
         lexer->lookahead != '\r') {
    if (interpolated && lexer->lookahead == '$') {
      lexer->advance(lexer, false);
      if (lexer->lookahead == '{')
        return has_content;
    } else {
      lexer->advance(lexer, false);
    }
    has_content = true;
    lexer->mark_end(lexer);
  }
  return has_content;
}

// String modes are carried by the grammar, so edits never need to restore a
// separate scanner stack when they enter a nested interpolation or raw string.
static bool scan_raw_content(TSLexer *lexer, bool interpolated) {
  bool has_content = false;
  lexer->mark_end(lexer);
  while (!lexer->eof(lexer)) {
    if (lexer->lookahead == '"') {
      unsigned quotes = 0;
      while (lexer->lookahead == '"' && quotes < 3) {
        lexer->advance(lexer, false);
        quotes++;
      }
      if (quotes == 3)
        return has_content;
    } else if (interpolated && lexer->lookahead == '$') {
      lexer->advance(lexer, false);
      if (lexer->lookahead == '{')
        return has_content;
    } else {
      lexer->advance(lexer, false);
    }
    has_content = true;
    lexer->mark_end(lexer);
  }
  return has_content;
}

static bool is_space(int32_t character) {
  return character == ' ' || character == '\t' || character == '\n' ||
         character == '\r' || character == '\f';
}

// The first slash has already been consumed. Block comments do not nest.
static bool skip_comment(TSLexer *lexer, bool *newline) {
  if (lexer->lookahead == '/') {
    while (!lexer->eof(lexer) && lexer->lookahead != '\n' &&
           lexer->lookahead != '\r')
      lexer->advance(lexer, false);
    return true;
  }
  if (lexer->lookahead != '*')
    return false;
  lexer->advance(lexer, false);
  bool star = false;
  while (!lexer->eof(lexer)) {
    if (star && lexer->lookahead == '/') {
      lexer->advance(lexer, false);
      break;
    }
    if (newline)
      *newline |= lexer->lookahead == '\n' || lexer->lookahead == '\r';
    star = lexer->lookahead == '*';
    lexer->advance(lexer, false);
  }
  return true;
}

static bool skip_trivia(TSLexer *lexer, bool *newline) {
  for (;;) {
    if (is_space(lexer->lookahead)) {
      if (newline)
        *newline |= lexer->lookahead == '\n' || lexer->lookahead == '\r';
      lexer->advance(lexer, false);
    } else if (lexer->lookahead == '/') {
      lexer->advance(lexer, false);
      if (!skip_comment(lexer, newline))
        return false;
    } else {
      return true;
    }
  }
}

// Both a guarded when arm and an else-if branch begin with `else if`.
// Only an arrow outside the guard's delimiters starts a new arm.
static bool else_starts_arm(TSLexer *lexer) {
  if (!skip_trivia(lexer, NULL))
    return false;
  if (lexer->lookahead == '-') {
    lexer->advance(lexer, false);
    return lexer->lookahead == '>';
  }
  if (lexer->lookahead != 'i')
    return false;
  lexer->advance(lexer, false);
  if (lexer->lookahead != 'f')
    return false;
  lexer->advance(lexer, false);
  if (!is_space(lexer->lookahead) && lexer->lookahead != '(' &&
      lexer->lookahead != '/')
    return false;
  if (!skip_trivia(lexer, NULL))
    return false;
  if (lexer->lookahead != '(')
    return true;

  unsigned depth = 0;
  int32_t quote = 0;
  bool needs_operand = false;
  while (!lexer->eof(lexer)) {
    int32_t character = lexer->lookahead;
    lexer->advance(lexer, false);
    if (quote) {
      if (character == '\\' && !lexer->eof(lexer))
        lexer->advance(lexer, false);
      else if (character == quote)
        quote = 0;
      continue;
    }
    if (character == '/' && skip_comment(lexer, NULL)) {
      continue;
    } else if (character == '"' || character == '\'' || character == '`') {
      quote = character;
      needs_operand = false;
    } else if (character == '(' || character == '[' || character == '{') {
      depth++;
    } else if (character == ')' || character == ']' || character == '}') {
      if (depth == 0)
        return false;
      depth--;
      needs_operand = false;
    } else if (character == '-' && lexer->lookahead == '>' && depth == 0) {
      return true;
    } else if (character == ';' && depth == 0) {
      return false;
    } else if ((character == '\n' || character == '\r') && depth == 0 &&
               !needs_operand) {
      if (!skip_trivia(lexer, NULL))
        return false;
      if (lexer->lookahead != '-')
        return false;
      lexer->advance(lexer, false);
      return lexer->lookahead == '>';
    } else if (!is_space(character)) {
      needs_operand =
          character < 128 && strchr("+-*/%<>=&|?:,", character) != NULL;
    }
  }
  return false;
}

static bool continues_expression(TSLexer *lexer, bool arm) {
  if (lexer->lookahead == '!') {
    lexer->advance(lexer, false);
    if (lexer->lookahead != 'i')
      return !arm && (lexer->lookahead == '!' || lexer->lookahead == '=');
  }
  switch (lexer->lookahead) {
  case '.':
  case ',':
  case ':':
  case '?':
  case '+':
  case '-':
  case '*':
  case '/':
  case '%':
  case '<':
  case '>':
  case '=':
  case '&':
  case '|':
  case '(':
  case '[':
  case '{':
    return !arm;
  default:
    break;
  }
  char word[16];
  unsigned length = 0;
  while ((lexer->lookahead >= 'a' && lexer->lookahead <= 'z') &&
         length < sizeof(word) - 1) {
    word[length++] = (char)lexer->lookahead;
    lexer->advance(lexer, false);
  }
  word[length] = '\0';
  if ((lexer->lookahead >= 'a' && lexer->lookahead <= 'z') ||
      (lexer->lookahead >= 'A' && lexer->lookahead <= 'Z') ||
      (lexer->lookahead >= '0' && lexer->lookahead <= '9') ||
      lexer->lookahead == '_' || lexer->lookahead >= 128)
    return false;
  if (arm) {
    return strcmp(word, "else") == 0 && !else_starts_arm(lexer);
  }
  return strcmp(word, "is") == 0 || strcmp(word, "in") == 0 ||
         strcmp(word, "as") == 0 || strcmp(word, "else") == 0 ||
         strcmp(word, "catch") == 0 || strcmp(word, "finally") == 0 ||
         strcmp(word, "where") == 0 || strcmp(word, "by") == 0 ||
         strcmp(word, "get") == 0 || strcmp(word, "set") == 0;
}

bool tree_sitter_scoop_external_scanner_scan(void *payload, TSLexer *lexer,
                                             const bool *valid_symbols) {
  (void)payload;
  // During error recovery all tokens can be valid; a zero-width separator
  // must not prevent the parser from consuming the erroneous input.
  if (valid_symbols[AUTOMATIC_SEMICOLON] && valid_symbols[ARM_SEMICOLON] &&
      valid_symbols[RAW_STRING_CONTENT] && valid_symbols[RAW_FSTRING_CONTENT])
    return false;
  if (valid_symbols[RAW_FSTRING_CONTENT] || valid_symbols[RAW_STRING_CONTENT]) {
    bool interpolated = valid_symbols[RAW_FSTRING_CONTENT];
    lexer->result_symbol =
        interpolated ? RAW_FSTRING_CONTENT : RAW_STRING_CONTENT;
    return scan_raw_content(lexer, interpolated);
  }
  if (valid_symbols[FSTRING_CONTENT] || valid_symbols[STRING_CONTENT]) {
    bool interpolated = valid_symbols[FSTRING_CONTENT];
    lexer->result_symbol = interpolated ? FSTRING_CONTENT : STRING_CONTENT;
    return scan_string_content(lexer, interpolated);
  }
  if (!valid_symbols[AUTOMATIC_SEMICOLON] && !valid_symbols[ARM_SEMICOLON])
    return false;
  lexer->mark_end(lexer);
  bool newline = false;
  if (!skip_trivia(lexer, &newline))
    return false;
  lexer->result_symbol =
      valid_symbols[ARM_SEMICOLON] ? ARM_SEMICOLON : AUTOMATIC_SEMICOLON;
  if (lexer->eof(lexer) || lexer->lookahead == '}')
    return true;
  if (!newline || lexer->lookahead == ';')
    return false;
  return !continues_expression(lexer, valid_symbols[ARM_SEMICOLON]);
}

void *tree_sitter_scoop_external_scanner_create(void) { return NULL; }
void tree_sitter_scoop_external_scanner_destroy(void *payload) {
  (void)payload;
}
unsigned tree_sitter_scoop_external_scanner_serialize(void *payload,
                                                      char *buffer) {
  (void)payload;
  (void)buffer;
  return 0;
}
void tree_sitter_scoop_external_scanner_deserialize(void *payload,
                                                    const char *buffer,
                                                    unsigned length) {
  (void)payload;
  (void)buffer;
  (void)length;
}
