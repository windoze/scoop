#ifndef SCOOP_COMPRESSION_H
#define SCOOP_COMPRESSION_H

#define ZSTD_STATIC_LINKING_ONLY
#include <brotli/decode.h>
#include <brotli/encode.h>
#include <errno.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <zlib-ng.h>
#include <zstd.h>
#include <zstd_errors.h>

enum {
  COMP_INVALID = 1,
  COMP_TRUNCATED,
  COMP_DICT_REQUIRED,
  COMP_DICT_MISMATCH,
  COMP_OUTPUT_LIMIT,
  COMP_WINDOW_LIMIT,
  COMP_BACKEND,
  COMP_PARAMETER,
  COMP_MEMORY,
  COMP_OVERFLOW
};

typedef struct {
  int32_t format, encoding, level, window, strategy, checksum, concatenate;
  int64_t max_output, max_window;
} comp_options;

typedef struct {
  int64_t memory;
  int32_t kind, code;
} comp_info;

typedef struct {
  int64_t consumed, produced, memory;
  int32_t status, kind, code;
} comp_step;

typedef struct {
  size_t consumed, produced;
  int status;
} comp_progress;

typedef struct {
  comp_options options;
  size_t memory;
  int kind, code, finished, boundary, flush_done, has_dictionary;
  int64_t total_output;
  char detail[192];
  unsigned char output[8192];
  size_t output_size, output_read;
  struct {
    int enabled, takeover, feeding_tail, has_last;
    unsigned char tail[4], last_byte;
    size_t tail_size, tail_read;
  } message;
  union {
    struct {
      zng_stream stream;
      zng_gz_header header;
      unsigned char *name, *comment;
    } deflate;
    struct {
      BrotliEncoderState *encoder;
      BrotliDecoderState *decoder;
    } brotli;
    struct {
      ZSTD_CCtx *encoder;
      ZSTD_DCtx *decoder;
    } zstd;
  } backend;
} comp_codec;

void *comp_allocate(void *opaque, size_t size);
void comp_release(void *opaque, void *address);
void *comp_zallocate(void *opaque, unsigned int count, unsigned int size);
void comp_fail(comp_codec *codec, int kind, int code, const char *detail);
void comp_drain(comp_codec *codec, unsigned char *output, int64_t capacity,
                comp_step *step);
size_t comp_output_capacity(const comp_codec *codec);
void comp_save_output(comp_codec *codec, size_t produced);
int comp_zstd_error(comp_codec *codec, size_t code, int parameter);
void comp_deflate_open(comp_codec *codec);
void comp_brotli_open(comp_codec *codec);
void comp_zstd_open(comp_codec *codec, const void *dictionary, size_t size);
comp_progress comp_deflate_step(comp_codec *codec, const unsigned char *input,
                                size_t size, unsigned char *output,
                                size_t capacity, int action);
comp_progress comp_brotli_step(comp_codec *codec, const unsigned char *input,
                               size_t size, unsigned char *output,
                               size_t capacity, int action);
comp_progress comp_zstd_step(comp_codec *codec, const unsigned char *input,
                             size_t size, unsigned char *output,
                             size_t capacity, int action);
void scoop_compression_free(uint64_t handle);
uint64_t scoop_compression_new(comp_options options, const void *dictionary,
                               int64_t dictionary_size);
comp_info scoop_compression_info(uint64_t handle);
comp_step comp_message_process(comp_codec *codec, const unsigned char *input,
                               int64_t size, unsigned char *output,
                               int64_t capacity, int action);
comp_progress comp_message_encode(comp_codec *codec, const unsigned char *input,
                                  size_t size, size_t capacity, int action);
void comp_message_resume_encoding(comp_codec *codec);
comp_progress comp_message_decode(comp_codec *codec, const unsigned char *input,
                                  size_t size, size_t capacity, int action);

#endif
