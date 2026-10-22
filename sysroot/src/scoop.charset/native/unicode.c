#include "charset.h"
#include <unigbrk.h>
#include <unictype.h>
#include <unicase.h>
#include <uniwidth.h>

void scoop_charset_grapheme_breaks(const uint8_t *text, int64_t size, char *breaks) {
  u8_grapheme_breaks(text, (size_t)size, breaks);
}

int32_t scoop_charset_scalar_width(uint32_t scalar) {
  return scalar == 0 ? -1 : uc_width(scalar, "UTF-8");
}

bool scoop_charset_alphanumeric(uint32_t scalar) {
  return uc_is_alnum(scalar);
}

scoop_charset_text scoop_charset_case(const uint8_t *text, int64_t size, int32_t mode) {
  if (size == 0)
    return (scoop_charset_text){0};
  size_t count = 0;
  uint8_t *result = mode == 2 ? u8_casefold(text, (size_t)size, NULL, NULL, NULL, &count)
                    : mode == 1 ? u8_toupper(text, (size_t)size, NULL, NULL, NULL, &count)
                                : u8_tolower(text, (size_t)size, NULL, NULL, NULL, &count);
  if (!result)
    return (scoop_charset_text){NULL, 0, CHARSET_BACKEND, errno};
  if (count > INT64_MAX) {
    free(result);
    return (scoop_charset_text){NULL, 0, CHARSET_BACKEND, EOVERFLOW};
  }
  return (scoop_charset_text){(char *)result, (int64_t)count, 0, 0};
}

void scoop_charset_unicode_free(char *data) { free(data); }
