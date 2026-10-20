#include "compression.h"

uint64_t scoop_compression_new(comp_options options, const void *dictionary,
                               int64_t dictionary_size) {
  comp_codec *codec = calloc(1, sizeof(*codec));
  if (!codec)
    return 0;
  codec->memory = sizeof(*codec);
  codec->options = options;
  codec->has_dictionary = dictionary_size != 0;
  if (options.format < 0 || options.format > 4 || options.max_output < -1 ||
      options.max_window < -1 || options.max_window == 0 ||
      (options.format != 4 && (dictionary_size || options.max_window >= 0))) {
    comp_fail(codec, COMP_PARAMETER, 0,
              "invalid compression format, dictionary or limit options");
  } else if (options.format <= 2)
    comp_deflate_open(codec);
  else if (options.format == 3)
    comp_brotli_open(codec);
  else
    comp_zstd_open(codec, dictionary, (size_t)dictionary_size);
  return (uint64_t)(uintptr_t)codec;
}

void scoop_compression_free(uint64_t handle) {
  comp_codec *codec = (comp_codec *)(uintptr_t)handle;
  if (!codec)
    return;
  if (codec->options.format <= 2) {
    if (codec->backend.deflate.stream.state) {
      if (codec->options.encoding)
        zng_deflateEnd(&codec->backend.deflate.stream);
      else
        zng_inflateEnd(&codec->backend.deflate.stream);
    }
    comp_release(codec, codec->backend.deflate.name);
    comp_release(codec, codec->backend.deflate.comment);
  } else if (codec->options.format == 3) {
    BrotliEncoderDestroyInstance(codec->backend.brotli.encoder);
    BrotliDecoderDestroyInstance(codec->backend.brotli.decoder);
  } else if (codec->options.format == 4) {
    ZSTD_freeCCtx(codec->backend.zstd.encoder);
    ZSTD_freeDCtx(codec->backend.zstd.decoder);
  }
  free(codec);
}
