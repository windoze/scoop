#include "compression.h"

comp_step scoop_compression_process(uint64_t handle, const unsigned char *input,
                                    int64_t size, unsigned char *output,
                                    int64_t capacity, int32_t action) {
  comp_codec *codec = (comp_codec *)(uintptr_t)handle;
  if (codec->message.enabled)
    return comp_message_process(codec, input, size, output, capacity, action);
  comp_step result = {0};
  comp_drain(codec, output, capacity, &result);
  if (codec->kind)
    goto done;
  if (codec->output_size) {
    result.status = 1;
    goto done;
  }
  if (codec->finished) {
    result.status = 2;
    goto done;
  }
  if (codec->boundary) {
    if (!size) {
      if (action == 2) {
        codec->finished = 1;
        result.status = 2;
      }
      goto done;
    }
    if (codec->options.format == 0) {
      int code = zng_inflateReset(&codec->backend.deflate.stream);
      if (code != Z_OK) {
        comp_fail(codec, COMP_BACKEND, code, "cannot continue gzip member");
        goto done;
      }
    }
    codec->boundary = 0;
  }
  if (codec->options.encoding && action == 1 && codec->flush_done)
    goto done;
  if (result.produced == capacity) {
    result.status = 1;
    goto done;
  }
  if (size)
    codec->flush_done = 0;
  size_t input_size = size < 8192 ? (size_t)size : 8192;
  size_t output_size = comp_output_capacity(codec);
  comp_progress step;
  if (codec->options.format <= 2)
    step = comp_deflate_step(codec, input, input_size, codec->output,
                             output_size, action);
  else if (codec->options.format == 3)
    step = comp_brotli_step(codec, input, input_size, codec->output,
                            output_size, action);
  else
    step = comp_zstd_step(codec, input, input_size, codec->output, output_size,
                          action);
  result.consumed = (int64_t)step.consumed;
  comp_save_output(codec, step.produced);
  if (step.status == 2) {
    if (!codec->options.encoding && codec->options.concatenate) {
      codec->boundary = 1;
      if (action == 2 && result.consumed == size)
        codec->finished = 1;
    } else
      codec->finished = 1;
  } else if (!codec->options.encoding && action == 2 &&
             result.consumed == size && step.status == 0) {
    comp_fail(codec, COMP_TRUNCATED, 0,
              "compressed input ended before the stream was complete");
  }
  if (codec->options.encoding && action == 1 && step.status == 0)
    codec->flush_done = 1;
  comp_drain(codec, output, capacity, &result);
  result.status = codec->output_size ? 1 : (codec->finished ? 2 : step.status);
  if (result.status == 2 && !codec->finished)
    result.status = 0;
done:
  result.memory = (int64_t)codec->memory;
  result.kind = codec->kind;
  result.code = codec->code;
  return result;
}
