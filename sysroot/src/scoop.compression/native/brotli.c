#include "compression.h"

void comp_brotli_open(comp_codec *codec) {
  comp_options options = codec->options;
  if (options.encoding) {
    if (options.level < 0 || options.level > 11 || options.window < 10 ||
        options.window > 24 || options.strategy < 0 || options.strategy > 2) {
      comp_fail(codec, COMP_PARAMETER, 0,
                "Brotli quality must be 0..11 and windowBits 10..24");
      return;
    }
    BrotliEncoderState *state =
        BrotliEncoderCreateInstance(comp_allocate, comp_release, codec);
    codec->backend.brotli.encoder = state;
    if (!state) {
      comp_fail(codec, COMP_BACKEND, ENOMEM, "cannot allocate Brotli encoder");
      return;
    }
    if (!BrotliEncoderSetParameter(state, BROTLI_PARAM_QUALITY,
                                   options.level) ||
        !BrotliEncoderSetParameter(state, BROTLI_PARAM_LGWIN, options.window) ||
        !BrotliEncoderSetParameter(state, BROTLI_PARAM_MODE,
                                   options.strategy)) {
      comp_fail(codec, COMP_PARAMETER, 0, "invalid Brotli encoder parameter");
    }
  } else {
    codec->backend.brotli.decoder =
        BrotliDecoderCreateInstance(comp_allocate, comp_release, codec);
    if (!codec->backend.brotli.decoder)
      comp_fail(codec, COMP_BACKEND, ENOMEM, "cannot allocate Brotli decoder");
  }
}

comp_progress comp_brotli_step(comp_codec *codec, const unsigned char *input,
                               size_t size, unsigned char *output,
                               size_t capacity, int action) {
  size_t left = size, space = capacity;
  comp_progress result = {0};
  if (codec->options.encoding) {
    BrotliEncoderState *state = codec->backend.brotli.encoder;
    BrotliEncoderOperation operation =
        action == 2
            ? BROTLI_OPERATION_FINISH
            : (action == 1 ? BROTLI_OPERATION_FLUSH : BROTLI_OPERATION_PROCESS);
    if (!BrotliEncoderCompressStream(state, operation, &left, &input, &space,
                                     &output, NULL)) {
      comp_fail(codec, COMP_BACKEND, 0, "Brotli encoder failed");
    } else if (BrotliEncoderIsFinished(state))
      result.status = 2;
    else if (left || BrotliEncoderHasMoreOutput(state) || action == 2)
      result.status = 1;
  } else {
    BrotliDecoderState *state = codec->backend.brotli.decoder;
    BrotliDecoderResult code = BrotliDecoderDecompressStream(
        state, &left, &input, &space, &output, NULL);
    if (code == BROTLI_DECODER_RESULT_ERROR) {
      BrotliDecoderErrorCode error = BrotliDecoderGetErrorCode(state);
      comp_fail(codec,
                error <= BROTLI_DECODER_ERROR_ALLOC_CONTEXT_MODES
                    ? COMP_BACKEND
                    : COMP_INVALID,
                error, BrotliDecoderErrorString(error));
    } else if (code == BROTLI_DECODER_RESULT_SUCCESS)
      result.status = 2;
    else if (code == BROTLI_DECODER_RESULT_NEEDS_MORE_OUTPUT)
      result.status = 1;
  }
  result.consumed = size - left;
  result.produced = capacity - space;
  return result;
}
