#include "compression.h"

static void drain(comp_codec *codec, unsigned char *output, int64_t capacity,
                  comp_step *step) {
  size_t available = codec->output_size - codec->output_read;
  size_t space = (size_t)(capacity - step->produced);
  size_t count = available < space ? available : space;
  if (count)
    memcpy(output + step->produced, codec->output + codec->output_read, count);
  codec->output_read += count;
  step->produced += (int64_t)count;
  if (codec->output_read == codec->output_size)
    codec->output_read = codec->output_size = 0;
}

comp_step scoop_compression_process(uint64_t handle, const unsigned char *input,
                                    int64_t size, unsigned char *output,
                                    int64_t capacity, int32_t action) {
  comp_codec *codec = (comp_codec *)(uintptr_t)handle;
  comp_step result = {0};
  drain(codec, output, capacity, &result);
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
  size_t output_size = sizeof(codec->output);
  int limited = !codec->options.encoding && codec->options.max_output >= 0;
  int64_t remaining = limited ? codec->options.max_output - codec->total_output
                              : INT64_MAX - codec->total_output;
  if (limited && remaining < (int64_t)output_size)
    output_size = (size_t)remaining + 1;
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
  if ((int64_t)step.produced > remaining) {
    step.produced = (size_t)remaining;
    comp_fail(codec, limited ? COMP_OUTPUT_LIMIT : COMP_OVERFLOW, 0,
              "decompressed output limit exceeded");
  }
  codec->total_output += (int64_t)step.produced;
  codec->output_size = step.produced;
  codec->output_read = 0;
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
  drain(codec, output, capacity, &result);
  result.status = codec->output_size ? 1 : (codec->finished ? 2 : step.status);
  if (result.status == 2 && !codec->finished)
    result.status = 0;
done:
  result.memory = (int64_t)codec->memory;
  result.kind = codec->kind;
  result.code = codec->code;
  return result;
}
