#include "compression.h"

uint64_t scoop_compression_message_new(comp_options options, int32_t takeover) {
  uint64_t handle = scoop_compression_new(options, NULL, 0);
  if (handle) {
    comp_codec *codec = (comp_codec *)(uintptr_t)handle;
    codec->message.enabled = 1;
    codec->message.takeover = takeover;
  }
  return handle;
}

comp_info scoop_compression_next_message(uint64_t handle) {
  comp_codec *codec = (comp_codec *)(uintptr_t)handle;
  if (!codec->message.enabled || !codec->finished || codec->output_size) {
    comp_fail(codec, COMP_PARAMETER, 0,
              "finish the current DEFLATE message first");
    return scoop_compression_info(handle);
  }
  int takeover = codec->message.takeover;
  int code = Z_OK;
  zng_stream *stream = &codec->backend.deflate.stream;
  if (!codec->options.encoding)
    code = takeover ? zng_inflateResetKeep(stream) : zng_inflateReset(stream);
  else if (!takeover)
    code = zng_deflateReset(stream);
  else
    comp_message_resume_encoding(codec);
  if (code != Z_OK)
    comp_fail(codec, COMP_BACKEND, code,
              "cannot start the next DEFLATE message");
  if (!codec->kind) {
    memset(&codec->message, 0, sizeof(codec->message));
    codec->message.enabled = 1;
    codec->message.takeover = takeover;
    codec->finished = codec->boundary = codec->flush_done = 0;
    codec->total_output = 0;
  }
  return scoop_compression_info(handle);
}

comp_step comp_message_process(comp_codec *codec, const unsigned char *input,
                               int64_t size, unsigned char *output,
                               int64_t capacity, int action) {
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
  if (action == 1 && !codec->options.encoding) {
    comp_fail(codec, COMP_PARAMETER, 0,
              "use finishMessage for a DEFLATE message");
    goto done;
  }
  if (action == 1 && codec->flush_done)
    goto done;
  if (result.produced == capacity) {
    result.status = 1;
    goto done;
  }
  if (size)
    codec->flush_done = 0;
  size_t input_size = size < 8192 ? (size_t)size : 8192;
  size_t output_size = comp_output_capacity(codec);
  comp_progress step =
      codec->options.encoding
          ? comp_message_encode(codec, input, input_size, output_size, action)
          : comp_message_decode(codec, input, input_size, output_size, action);
  result.consumed = (int64_t)step.consumed;
  comp_save_output(codec, step.produced);
  if (step.status == 2)
    codec->finished = 1;
  if (codec->options.encoding && action == 1 && step.status == 0)
    codec->flush_done = 1;
  comp_drain(codec, output, capacity, &result);
  result.status = codec->output_size ? 1 : (codec->finished ? 2 : step.status);
done:
  result.memory = (int64_t)codec->memory;
  result.kind = codec->kind;
  result.code = codec->code;
  return result;
}
