#include "compression.h"

void comp_drain(comp_codec *codec, unsigned char *output, int64_t capacity,
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

size_t comp_output_capacity(const comp_codec *codec) {
  size_t capacity = sizeof(codec->output);
  if (!codec->options.encoding && codec->options.max_output >= 0) {
    int64_t remaining = codec->options.max_output - codec->total_output;
    if (remaining < (int64_t)capacity)
      capacity = (size_t)remaining + 1;
  }
  return capacity;
}

void comp_save_output(comp_codec *codec, size_t produced) {
  int limited = !codec->options.encoding && codec->options.max_output >= 0;
  int64_t remaining = limited ? codec->options.max_output - codec->total_output
                              : INT64_MAX - codec->total_output;
  if ((int64_t)produced > remaining) {
    produced = (size_t)remaining;
    comp_fail(codec, limited ? COMP_OUTPUT_LIMIT : COMP_OVERFLOW, 0,
              "decompressed output limit exceeded");
  }
  codec->total_output += (int64_t)produced;
  codec->output_size = produced;
  codec->output_read = 0;
}
