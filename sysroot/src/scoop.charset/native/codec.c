#include "charset.h"

int scoop_charset_decode(scoop_charset *state, unsigned char scalar[4],
                         int64_t *offset) {
  *offset = state->unit_offset;
  if (state->input_size) {
    char *input = (char *)state->input, *output = (char *)scalar;
    size_t left = state->input_size, space = 4;
    size_t code = iconv(state->decoder, &input, &left, &output, &space);
    int error = code == (size_t)-1 ? errno : 0;
    scoop_charset_consume(state, state->input_size - left);
    if (error == EILSEQ)
      scoop_charset_fail(state, CHARSET_INVALID, error, state->offset);
    else if (error && error != E2BIG && error != EINVAL)
      scoop_charset_fail(state, CHARSET_BACKEND, error, state->offset);
    if (space == 0) {
      state->unit_offset = state->offset;
      return 1;
    }
    if (space != 4 || error == E2BIG)
      scoop_charset_fail(state, CHARSET_BACKEND, E2BIG, state->offset);
    if (state->error)
      return 0;
    if (error == EINVAL) {
      if (!state->ending)
        return 0;
      if (!state->options.replace_input) {
        scoop_charset_fail(state, CHARSET_INCOMPLETE, error, state->offset);
        return 0;
      }
      state->tail_replacement = 1;
      state->tail_offset = state->offset;
      scoop_charset_consume(state, state->input_size);
    }
  }
  if (!state->ending)
    return 0;
  if (!state->decoder_done) {
    char *output = (char *)scalar;
    size_t space = 4;
    size_t code = iconv(state->decoder, NULL, NULL, &output, &space);
    if (code == (size_t)-1) {
      scoop_charset_fail(state, CHARSET_BACKEND, errno, state->offset);
      return 0;
    }
    state->decoder_done = 1;
    if (space == 0)
      return 1;
    if (space != 4) {
      scoop_charset_fail(state, CHARSET_BACKEND, EINVAL, state->offset);
      return 0;
    }
  }
  if (state->tail_replacement) {
    state->tail_replacement = 0;
    *offset = state->tail_offset;
    memcpy(scalar, "\xFD\xFF\0\0", 4);
    return 1;
  }
  return 2;
}

void scoop_charset_encode(scoop_charset *state, unsigned char scalar[4],
                          int64_t offset) {
  char *input = (char *)scalar, *output = (char *)state->output;
  size_t left = 4, space = sizeof(state->output);
  size_t code = iconv(state->encoder, &input, &left, &output, &space);
  if (code == (size_t)-1 && errno == EILSEQ && state->options.replace_output) {
    memcpy(scalar, "?\0\0\0", 4);
    input = (char *)scalar;
    left = 4;
    code = iconv(state->encoder, &input, &left, &output, &space);
  }
  if (code == (size_t)-1 || code != 0 || left) {
    int error = code == (size_t)-1 ? errno : EILSEQ;
    scoop_charset_fail(state,
                       error == EILSEQ ? CHARSET_UNMAPPABLE : CHARSET_BACKEND,
                       error, offset);
    return;
  }
  state->output_size = sizeof(state->output) - space;
  state->output_read = 0;
}

void scoop_charset_finish(scoop_charset *state) {
  char *output = (char *)state->output;
  size_t space = sizeof(state->output);
  if (iconv(state->encoder, NULL, NULL, &output, &space) == (size_t)-1) {
    scoop_charset_fail(state, CHARSET_BACKEND, errno, state->offset);
    return;
  }
  state->output_size = sizeof(state->output) - space;
  state->output_read = 0;
  state->encoder_done = 1;
}
