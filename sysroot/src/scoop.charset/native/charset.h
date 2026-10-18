#ifndef SCOOP_CHARSET_H
#define SCOOP_CHARSET_H

#include <errno.h>
#include <iconv.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

enum {
  CHARSET_UNKNOWN = 1,
  CHARSET_MEMORY,
  CHARSET_INVALID,
  CHARSET_UNMAPPABLE,
  CHARSET_INCOMPLETE,
  CHARSET_MISSING_BOM,
  CHARSET_CONFLICTING_BOM,
  CHARSET_OPTION,
  CHARSET_BACKEND
};

typedef struct {
  int32_t input_bom, source_order, destination_order, emit_bom;
  int32_t replace_input, replace_output;
} scoop_charset_options;

typedef struct {
  char *data;
  int64_t size;
  int32_t code, backend;
} scoop_charset_text;

typedef struct {
  uint64_t handle;
  int32_t code, backend;
} scoop_charset_result;

typedef struct {
  int64_t consumed, produced, offset;
  int32_t status, code, backend;
} scoop_charset_step;

typedef struct {
  iconv_t decoder, encoder;
  char *source, *destination;
  scoop_charset_options options;
  unsigned char input[64], output[64];
  size_t input_size, output_size, output_read;
  int source_kind, ending, header_done, decoder_done, encoder_done,
      tail_replacement;
  int error, backend;
  int64_t offset, unit_offset, tail_offset, error_offset;
} scoop_charset;

char *scoop_charset_copy(const char *data, size_t size);
int scoop_charset_kind(const char *name);
const char *scoop_charset_ordered(int kind, int order);
size_t scoop_charset_bom(int kind, int order, unsigned char *bytes);
void scoop_charset_fail(scoop_charset *state, int code, int backend,
                        int64_t offset);
void scoop_charset_consume(scoop_charset *state, size_t count);
int scoop_charset_header(scoop_charset *state);
int scoop_charset_decode(scoop_charset *state, unsigned char scalar[4],
                         int64_t *offset);
void scoop_charset_encode(scoop_charset *state, unsigned char scalar[4],
                          int64_t offset);
void scoop_charset_finish(scoop_charset *state);
void scoop_charset_free(uint64_t handle);

#endif
