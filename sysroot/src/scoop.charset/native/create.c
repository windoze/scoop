#include "charset.h"

int scoop_charset_kind(const char *name) {
  const char *names[] = {"",         "UTF-8",  "UTF-16",   "UTF-16BE",
                         "UTF-16LE", "UTF-32", "UTF-32BE", "UTF-32LE"};
  for (int i = 1; i < 8; i++) {
    if (!strcmp(name, names[i]))
      return i;
  }
  return 0;
}

const char *scoop_charset_ordered(int kind, int order) {
  if (kind == 2)
    return order == 1 ? "UTF-16BE" : "UTF-16LE";
  return order == 1 ? "UTF-32BE" : "UTF-32LE";
}

void scoop_charset_free(uint64_t handle) {
  scoop_charset *state = (scoop_charset *)(uintptr_t)handle;
  if (!state)
    return;
  if (state->decoder != (iconv_t)-1)
    iconv_close(state->decoder);
  if (state->encoder != (iconv_t)-1)
    iconv_close(state->encoder);
  free(state->source);
  free(state->destination);
  free(state);
}

void scoop_charset_fail(scoop_charset *state, int code, int backend,
                        int64_t offset) {
  if (!state->error || offset < state->error_offset) {
    state->error = code;
    state->backend = backend;
    state->error_offset = offset;
  }
}

void scoop_charset_consume(scoop_charset *state, size_t count) {
  if (count > (uint64_t)(INT64_MAX - state->offset)) {
    scoop_charset_fail(state, CHARSET_BACKEND, EOVERFLOW, state->offset);
    return;
  }
  state->offset += (int64_t)count;
  state->input_size -= count;
  memmove(state->input, state->input + count, state->input_size);
}

scoop_charset_result scoop_charset_new(const char *source, int64_t source_size,
                                       const char *destination,
                                       int64_t destination_size,
                                       scoop_charset_options options) {
  scoop_charset *state = calloc(1, sizeof(*state));
  if (!state)
    return (scoop_charset_result){0, CHARSET_MEMORY, ENOMEM};
  state->decoder = state->encoder = (iconv_t)-1;
  state->source = scoop_charset_copy(source, (size_t)source_size);
  state->destination =
      scoop_charset_copy(destination, (size_t)destination_size);
  state->options = options;
  int error = CHARSET_MEMORY, backend = ENOMEM;
  if (!state->source || !state->destination)
    goto fail;
  state->source_kind = scoop_charset_kind(state->source);
  int kind = scoop_charset_kind(state->destination);
  int source_ambiguous = state->source_kind == 2 || state->source_kind == 5;
  int destination_ambiguous = kind == 2 || kind == 5;
  if ((options.source_order && !source_ambiguous) ||
      (options.destination_order && !destination_ambiguous) ||
      (destination_ambiguous && !options.destination_order) ||
      (!state->source_kind && options.input_bom == 2) ||
      (!kind && options.emit_bom)) {
    error = CHARSET_OPTION;
    backend = 0;
    goto fail;
  }
  const char *target =
      destination_ambiguous
          ? scoop_charset_ordered(kind, options.destination_order)
          : state->destination;
  state->encoder = iconv_open(target, "UTF-32LE");
  if (state->encoder == (iconv_t)-1) {
    error = CHARSET_BACKEND;
    backend = errno;
    goto fail;
  }
  if (options.emit_bom)
    state->output_size =
        scoop_charset_bom(kind, options.destination_order, state->output);
  return (scoop_charset_result){(uint64_t)(uintptr_t)state, 0, 0};
fail:
  scoop_charset_free((uint64_t)(uintptr_t)state);
  return (scoop_charset_result){0, error, backend};
}
