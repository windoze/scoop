#include "charset.h"

scoop_charset_step
scoop_charset_process(uint64_t handle, const unsigned char *input,
                      int64_t input_size, unsigned char *output,
                      int64_t output_size, int32_t end_input) {
  scoop_charset *state = (scoop_charset *)(uintptr_t)handle;
  scoop_charset_step result = {0};
  while (1) {
    size_t pending = state->output_size - state->output_read;
    size_t space = (size_t)(output_size - result.produced);
    size_t count = pending < space ? pending : space;
    if (count) {
      memcpy(output + result.produced, state->output + state->output_read,
             count);
      state->output_read += count;
      result.produced += (int64_t)count;
    }
    if (state->output_read < state->output_size) {
      result.status = 1;
      break;
    }
    state->output_read = state->output_size = 0;
    if (state->error)
      break;
    if (state->encoder_done) {
      result.status = 2;
      break;
    }
    if (result.produced == output_size) {
      result.status = 1;
      break;
    }

    size_t available = (size_t)(input_size - result.consumed);
    size_t room = sizeof(state->input) - state->input_size;
    count = available < room ? available : room;
    if (count) {
      memcpy(state->input + state->input_size, input + result.consumed, count);
      state->input_size += count;
      result.consumed += (int64_t)count;
    }
    if (end_input && result.consumed == input_size)
      state->ending = 1;
    if (!state->header_done && !scoop_charset_header(state))
      break;
    unsigned char scalar[4];
    int64_t offset;
    int decoded = scoop_charset_decode(state, scalar, &offset);
    if (decoded == 1)
      scoop_charset_encode(state, scalar, offset);
    else if (decoded == 2)
      scoop_charset_finish(state);
    else {
      if (state->error)
        break;
      if (result.consumed == input_size)
        break;
      if (state->input_size == sizeof(state->input)) {
        scoop_charset_fail(state, CHARSET_BACKEND, EOVERFLOW, state->offset);
        break;
      }
    }
  }
  result.code = state->error;
  result.backend = state->backend;
  result.offset = state->error_offset;
  return result;
}
