#include "charset.h"

size_t scoop_charset_bom(int kind, int order, unsigned char *bytes) {
  if (kind == 1) {
    memcpy(bytes, "\xEF\xBB\xBF", 3);
    return 3;
  }
  if (kind == 3 || kind == 6)
    order = 1;
  if (kind == 4 || kind == 7)
    order = 2;
  if (kind >= 2 && kind <= 4) {
    memcpy(bytes, order == 1 ? "\xFE\xFF" : "\xFF\xFE", 2);
    return 2;
  }
  if (kind >= 5) {
    memcpy(bytes, order == 1 ? "\0\0\xFE\xFF" : "\xFF\xFE\0\0", 4);
    return 4;
  }
  return 0;
}

static void replace_input(const char *bytes, size_t count,
                          void (*write)(const unsigned int *, size_t, void *),
                          void *context, void *data) {
  (void)bytes;
  (void)count;
  (void)data;
  const unsigned int replacement = 0xFFFD;
  write(&replacement, 1, context);
}

int scoop_charset_header(scoop_charset *state) {
  int kind = state->source_kind;
  int ambiguous = kind == 2 || kind == 5;
  int order = state->options.source_order;
  const char *source = state->source;
  if (kind) {
    unsigned char big[4], little[4];
    int family = kind == 1 ? 1 : (kind <= 4 ? 2 : 5);
    size_t size = scoop_charset_bom(family, 1, big);
    scoop_charset_bom(family, 2, little);
    size_t prefix = state->input_size < size ? state->input_size : size;
    int matches_big = !memcmp(state->input, big, prefix);
    int matches_little = !memcmp(state->input, little, prefix);
    if (prefix < size && !state->ending && (matches_big || matches_little))
      return 0;
    int present = prefix == size && (matches_big || matches_little);
    int found_order = matches_big ? 1 : 2;
    int required_order =
        kind == 3 || kind == 6 ? 1 : (kind == 4 || kind == 7 ? 2 : 0);
    if (present && required_order && required_order != found_order) {
      scoop_charset_fail(state, CHARSET_CONFLICTING_BOM, 0, 0);
      return 0;
    }
    if ((!present && state->options.input_bom == 2) ||
        (ambiguous && !present && !order)) {
      scoop_charset_fail(state, CHARSET_MISSING_BOM, 0, 0);
      return 0;
    }
    if (ambiguous)
      source = scoop_charset_ordered(kind, present ? found_order : order);
    if (present && state->options.input_bom != 1)
      scoop_charset_consume(state, size);
  }
  state->unit_offset = state->offset;
  state->decoder = iconv_open("UTF-32LE", source);
  if (state->decoder == (iconv_t)-1) {
    scoop_charset_fail(state, CHARSET_BACKEND, errno, state->offset);
    return 0;
  }
  if (state->options.replace_input) {
    struct iconv_fallbacks fallbacks = {0};
    fallbacks.mb_to_uc_fallback = replace_input;
    if (iconvctl(state->decoder, ICONV_SET_FALLBACKS, &fallbacks)) {
      scoop_charset_fail(state, CHARSET_BACKEND, errno, state->offset);
      return 0;
    }
  }
  state->header_done = 1;
  return 1;
}
