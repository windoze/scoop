#ifndef SCOOP_TEXT_BACKEND_H
#define SCOOP_TEXT_BACKEND_H

#include <stdbool.h>
#include <stdint.h>

typedef struct {
  uint8_t *data;
  int64_t size;
  int32_t code;
  int64_t label;
} scoop_idna_result;

typedef struct {
  uint8_t *ascii, *unicode;
  int64_t ascii_size, unicode_size;
  int32_t code;
  int64_t label;
} scoop_idna_name;

typedef struct {
  uint8_t *data;
  int64_t size;
  int32_t code, backend;
} scoop_text_result;

/* Input spans are readable for the synchronous call. A zero size permits null.
 * Owned output must be released once with the exact returned pointer and size;
 * empty output is null/zero. Never pass Rust-owned output to C free.
 * Invalid input returns a status; internal panic or allocation failure aborts. */
void scoop_text_free(uint8_t *data, int64_t size);
const char *scoop_text_unicode_version(void);
const char *scoop_idna_version(void);
const char *scoop_idna_message(int32_t code);
scoop_idna_result scoop_idna_domain(const uint8_t *, int64_t, int32_t mode);
scoop_idna_name scoop_idna_parse(const uint8_t *, int64_t);
scoop_idna_result scoop_idna_punycode(const uint8_t *, int64_t, bool decode);
scoop_text_result scoop_charset_normalize_nfc(const uint8_t *, int64_t);

#endif
