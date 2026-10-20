#include "compression.h"

void comp_deflate_open(comp_codec *codec) {
  comp_options options = codec->options;
  if (options.window < (options.encoding ? 9 : 8) || options.window > 15 ||
      (options.encoding && (options.level < 0 || options.level > 9 ||
                            options.strategy < 0 || options.strategy > 4))) {
    comp_fail(codec, COMP_PARAMETER, 0,
              "DEFLATE level must be 0..9 and encoding windowBits 9..15");
    return;
  }
  int window = options.format == 0
                   ? options.window + 16
                   : (options.format == 2 ? -options.window : options.window);
  zng_stream *stream = &codec->backend.deflate.stream;
  stream->zalloc = comp_zallocate;
  stream->zfree = comp_release;
  stream->opaque = codec;
  int code = options.encoding
                 ? zng_deflateInit2(stream, options.level, Z_DEFLATED, window,
                                    8, options.strategy)
                 : zng_inflateInit2(stream, window);
  if (code != Z_OK) {
    comp_fail(codec, COMP_BACKEND, code, "cannot initialize DEFLATE");
    return;
  }
  if (options.encoding && options.format == 0) {
    codec->backend.deflate.header.os = 255;
    code = zng_deflateSetHeader(stream, &codec->backend.deflate.header);
    if (code != Z_OK)
      comp_fail(codec, COMP_BACKEND, code, "cannot initialize gzip header");
  }
}

static unsigned char *gzip_text(comp_codec *codec, const unsigned char *data,
                                size_t size) {
  if (!data)
    return NULL;
  if (memchr(data, 0, size)) {
    comp_fail(codec, COMP_PARAMETER, 0, "gzip metadata cannot contain NUL");
    return NULL;
  }
  if (size == SIZE_MAX) {
    comp_fail(codec, COMP_BACKEND, ENOMEM, "gzip metadata is too large");
    return NULL;
  }
  unsigned char *copy = comp_allocate(codec, size + 1);
  if (!copy) {
    comp_fail(codec, COMP_BACKEND, ENOMEM, "cannot allocate gzip metadata");
    return NULL;
  }
  memcpy(copy, data, size);
  copy[size] = 0;
  return copy;
}

void scoop_compression_gzip(uint64_t handle, const unsigned char *name,
                            int64_t name_size, const unsigned char *comment,
                            int64_t comment_size, uint32_t mtime,
                            int32_t hcrc) {
  comp_codec *codec = (comp_codec *)(uintptr_t)handle;
  codec->backend.deflate.name = gzip_text(codec, name, (size_t)name_size);
  codec->backend.deflate.comment =
      gzip_text(codec, comment, (size_t)comment_size);
  zng_gz_header *header = &codec->backend.deflate.header;
  header->name = codec->backend.deflate.name;
  header->comment = codec->backend.deflate.comment;
  header->time = mtime;
  header->hcrc = hcrc;
}

comp_progress comp_deflate_step(comp_codec *codec, const unsigned char *input,
                                size_t size, unsigned char *output,
                                size_t capacity, int action) {
  zng_stream *stream = &codec->backend.deflate.stream;
  stream->next_in = input;
  stream->avail_in = (uint32_t)size;
  stream->next_out = output;
  stream->avail_out = (uint32_t)capacity;
  int code = codec->options.encoding
                 ? zng_deflate(stream, action == 2 ? Z_FINISH
                                                   : (action == 1 ? Z_SYNC_FLUSH
                                                                  : Z_NO_FLUSH))
                 : zng_inflate(stream, Z_NO_FLUSH);
  comp_progress result = {size - stream->avail_in, capacity - stream->avail_out,
                          0};
  if (code == Z_STREAM_END)
    result.status = 2;
  else if (code == Z_NEED_DICT)
    comp_fail(codec, COMP_DICT_REQUIRED, code,
              "DEFLATE stream requires a preset dictionary");
  else if (code != Z_OK && code != Z_BUF_ERROR) {
    comp_fail(codec, code == Z_DATA_ERROR ? COMP_INVALID : COMP_BACKEND, code,
              stream->msg ? stream->msg : "DEFLATE codec failed");
  } else if (!stream->avail_out || stream->avail_in ||
             (codec->options.encoding && action == 2))
    result.status = 1;
  stream->next_in = NULL;
  stream->next_out = NULL;
  return result;
}
