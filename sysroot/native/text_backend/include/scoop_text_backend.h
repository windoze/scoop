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
/* Grapheme output is a disjoint writable span of input-size bytes. A value of
 * one marks the first byte of a cluster; the end boundary is implicit. Empty
 * spans permit null. Return zero on success or Charset.Backend (9) on error. */
int32_t scoop_charset_grapheme_breaks(const uint8_t *, int64_t, uint8_t *);
int32_t scoop_charset_scalar_width(uint32_t);
bool scoop_charset_alphanumeric(uint32_t);
/* Modes: 0 = root lowercase, 1 = root uppercase, 2 = default full case fold. */
scoop_text_result scoop_charset_case(const uint8_t *, int64_t, int32_t);

typedef struct {
  int32_t input_bom, source_order, destination_order, emit_bom, replace_input, replace_output;
} scoop_charset_options;
typedef struct { uint64_t handle; int32_t code, backend; } scoop_charset_result;
typedef struct {
  int64_t consumed, produced, offset;
  int32_t status, code, backend;
} scoop_charset_step;

const char *scoop_charset_version(void);
scoop_text_result scoop_charset_name(const uint8_t *, int64_t);
scoop_text_result scoop_charset_names(void);
scoop_charset_result scoop_charset_new(const uint8_t *, int64_t, const uint8_t *, int64_t, scoop_charset_options);
/* Handles are exclusively owned and calls must not overlap. Input/output spans
 * must be disjoint. No pointer is retained after process; free once after use. */
scoop_charset_step scoop_charset_process(uint64_t, const uint8_t *, int64_t, uint8_t *, int64_t, int32_t);
void scoop_charset_free(uint64_t);

#endif
