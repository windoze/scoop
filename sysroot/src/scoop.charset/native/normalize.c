#include "charset.h"
#include <uninorm.h>

scoop_charset_text scoop_charset_normalize_nfc(const uint8_t *data,
                                               int64_t size) {
  if (size == 0)
    return (scoop_charset_text){0};
  size_t count = 0;
  uint8_t *result = u8_normalize(UNINORM_NFC, data, (size_t)size, NULL, &count);
  if (!result)
    return (scoop_charset_text){NULL, 0, CHARSET_BACKEND, errno};
  if (count > INT64_MAX) {
    free(result);
    return (scoop_charset_text){NULL, 0, CHARSET_BACKEND, EOVERFLOW};
  }
  return (scoop_charset_text){(char *)result, (int64_t)count, 0, 0};
}
