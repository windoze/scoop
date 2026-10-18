#include "idna.h"
#include <errno.h>
#include <unistr.h>

extern int scoop_libidn2_punycode_decode_raw(const char *, size_t, uint32_t *,
                                             size_t *);

static scoop_idna_result conversion_error(void) {
  return scoop_idna_error(
      errno == ENOMEM ? SCOOP_IDNA_ALLOC : SCOOP_IDNA_SCALAR, -1);
}

static scoop_idna_result encode(const uint8_t *data, size_t size) {
  size_t length;
  uint32_t *input = u8_to_u32(data, size, NULL, &length);
  if (!input)
    return conversion_error();
  size_t capacity = size + 16;
  uint8_t *output = NULL;
  for (;;) {
    uint8_t *next = realloc(output, capacity);
    if (!next) {
      free(output);
      free(input);
      return scoop_idna_error(SCOOP_IDNA_ALLOC, -1);
    }
    output = next;
    size_t used = capacity;
    int code = idn2_punycode_encode(input, length, (char *)output, &used);
    if (!code) {
      free(input);
      return scoop_idna_bytes(output, used);
    }
    if (code != IDN2_PUNYCODE_BIG_OUTPUT || capacity > SIZE_MAX / 2) {
      free(input);
      free(output);
      return scoop_idna_error(code, -1);
    }
    capacity *= 2;
  }
}

static scoop_idna_result decode(const uint8_t *data, size_t size) {
  if (size >= SIZE_MAX / sizeof(uint32_t))
    return scoop_idna_error(SCOOP_IDNA_ALLOC, -1);
  size_t length = size + 1;
  uint32_t *decoded = malloc(length * sizeof(uint32_t));
  if (!decoded)
    return scoop_idna_error(SCOOP_IDNA_ALLOC, -1);
  int code =
      scoop_libidn2_punycode_decode_raw((char *)data, size, decoded, &length);
  if (code) {
    free(decoded);
    return scoop_idna_error(code, -1);
  }
  size_t count;
  uint8_t *output = u32_to_u8(decoded, length, NULL, &count);
  int saved_errno = errno;
  free(decoded);
  if (!output) {
    return scoop_idna_error(
        saved_errno == ENOMEM ? SCOOP_IDNA_ALLOC : SCOOP_IDNA_SCALAR, -1);
  }
  return scoop_idna_bytes(output, count);
}

scoop_idna_result scoop_idna_punycode(const uint8_t *data, int64_t size,
                                      bool decoding) {
  if (!size)
    return scoop_idna_bytes(NULL, 0);
  return decoding ? decode(data, (size_t)size) : encode(data, (size_t)size);
}
