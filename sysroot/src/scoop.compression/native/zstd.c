#include "compression.h"

int comp_zstd_error(comp_codec *codec, size_t code, int parameter) {
  if (!ZSTD_isError(code))
    return 0;
  ZSTD_ErrorCode error = ZSTD_getErrorCode(code);
  int kind = COMP_INVALID;
  if (parameter)
    kind = COMP_PARAMETER;
  else if (error == ZSTD_error_frameParameter_windowTooLarge)
    kind = COMP_WINDOW_LIMIT;
  else if (error == ZSTD_error_dictionary_wrong)
    kind = codec->has_dictionary ? COMP_DICT_MISMATCH : COMP_DICT_REQUIRED;
  else if (error == ZSTD_error_memory_allocation || error == ZSTD_error_GENERIC)
    kind = COMP_BACKEND;
  comp_fail(codec, kind, (int)error, ZSTD_getErrorName(code));
  return 1;
}

void comp_zstd_open(comp_codec *codec, const void *dictionary, size_t size) {
  ZSTD_customMem memory = {comp_allocate, comp_release, codec};
  if (codec->options.encoding) {
    ZSTD_bounds bounds = ZSTD_cParam_getBounds(ZSTD_c_compressionLevel);
    if (codec->options.level < bounds.lowerBound ||
        codec->options.level > bounds.upperBound) {
      comp_fail(codec, COMP_PARAMETER, 0,
                "zstd level is outside the supported range");
      return;
    }
    ZSTD_CCtx *state = ZSTD_createCCtx_advanced(memory);
    codec->backend.zstd.encoder = state;
    if (!state) {
      comp_fail(codec, COMP_BACKEND, ENOMEM, "cannot allocate zstd encoder");
      return;
    }
    if (comp_zstd_error(codec,
                        ZSTD_CCtx_setParameter(state, ZSTD_c_compressionLevel,
                                               codec->options.level),
                        1) ||
        comp_zstd_error(codec,
                        ZSTD_CCtx_setParameter(state, ZSTD_c_checksumFlag,
                                               codec->options.checksum),
                        1))
      return;
    comp_zstd_error(codec, ZSTD_CCtx_loadDictionary(state, dictionary, size),
                    0);
  } else {
    ZSTD_DCtx *state = ZSTD_createDCtx_advanced(memory);
    codec->backend.zstd.decoder = state;
    if (!state) {
      comp_fail(codec, COMP_BACKEND, ENOMEM, "cannot allocate zstd decoder");
      return;
    }
    if (codec->options.max_window >= 0 &&
        comp_zstd_error(codec,
                        ZSTD_DCtx_setMaxWindowSize(
                            state, (size_t)codec->options.max_window),
                        1))
      return;
    comp_zstd_error(codec, ZSTD_DCtx_loadDictionary(state, dictionary, size),
                    0);
  }
}

comp_progress comp_zstd_step(comp_codec *codec, const unsigned char *input,
                             size_t size, unsigned char *output,
                             size_t capacity, int action) {
  ZSTD_inBuffer in = {input, size, 0};
  ZSTD_outBuffer out = {output, capacity, 0};
  size_t code =
      codec->options.encoding
          ? ZSTD_compressStream2(
                codec->backend.zstd.encoder, &out, &in,
                action == 2 ? ZSTD_e_end
                            : (action == 1 ? ZSTD_e_flush : ZSTD_e_continue))
          : ZSTD_decompressStream(codec->backend.zstd.decoder, &out, &in);
  comp_progress result = {in.pos, out.pos, 0};
  if (comp_zstd_error(codec, code, 0))
    return result;
  if (!codec->options.encoding && code == 0)
    result.status = 2;
  else if (codec->options.encoding && action == 2 && code == 0)
    result.status = 2;
  else if (out.pos == capacity || in.pos < size ||
           (codec->options.encoding && action && code != 0))
    result.status = 1;
  return result;
}
