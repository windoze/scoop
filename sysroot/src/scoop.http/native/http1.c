#include "http1.h"
#include <stdlib.h>
#include <string.h>

int http1_fail(http1 *state, int code, const char *message) {
  http_fail(&state->error, code, message);
  llhttp_set_error_reason(&state->parser, state->error.text);
  return HPE_USER;
}

uint64_t scoop_http1_new(int32_t server, http1_config config) {
  http1 *state = calloc(1, sizeof(*state));
  if (!state)
    return 0;
  state->server = server;
  state->config = config;
  http1_callbacks(state);
  llhttp_init(&state->parser, server ? HTTP_REQUEST : HTTP_RESPONSE,
              &state->callbacks);
  state->parser.data = state;
  llhttp_t *p = &state->parser;
  llhttp_set_lenient_headers(p, !!(config.flags & 1));
  llhttp_set_lenient_chunked_length(p, !!(config.flags & 2));
  llhttp_set_lenient_transfer_encoding(p, !!(config.flags & 4));
  llhttp_set_lenient_version(p, !!(config.flags & 8));
  llhttp_set_lenient_optional_lf_after_cr(p, !!(config.flags & 16));
  llhttp_set_lenient_optional_cr_before_lf(p, !!(config.flags & 32));
  llhttp_set_lenient_optional_crlf_after_chunk(p, !!(config.flags & 64));
  llhttp_set_lenient_spaces_after_chunk_size(p, !!(config.flags & 128));
  llhttp_set_lenient_data_after_close(p, config.after_close == 1);
  llhttp_set_lenient_keep_alive(p, config.after_close == 2);
  return (uint64_t)(uintptr_t)state;
}

void scoop_http1_free(uint64_t handle) {
  http1 *state = (http1 *)(uintptr_t)handle;
  http_events_clear(&state->events);
  http_fields_clear(&state->fields);
  http_bytes_clear(&state->method);
  http_bytes_clear(&state->target);
  http_bytes_clear(&state->reason);
  http_bytes_clear(&state->protocol);
  http_bytes_clear(&state->field_name);
  http_bytes_clear(&state->field_value);
  while (state->first_method) {
    http1_method *method = state->first_method;
    state->first_method = method->next;
    free(method);
  }
  free(state);
}

int32_t scoop_http1_expect(uint64_t handle, const uint8_t *name,
                           int64_t length) {
  http1 *state = (http1 *)(uintptr_t)handle;
  if (state->server || state->failed || state->ended || state->switched)
    return http_fail(&state->error, -2,
                     "cannot register an HTTP/1 response here");
  http1_method *method = calloc(1, sizeof(*method));
  if (!method)
    return http_fail(&state->error, -1,
                     "cannot allocate HTTP/1 request context");
  /* Only response framing needs the method; the wire token stays in Scoop. */
  method->method = HTTP_GET;
  if (length == 4 && memcmp(name, "HEAD", 4) == 0)
    method->method = HTTP_HEAD;
  else if (length == 7 && memcmp(name, "CONNECT", 7) == 0)
    method->method = HTTP_CONNECT;
  if (state->last_method)
    state->last_method->next = method;
  else
    state->first_method = method;
  state->last_method = method;
  return 0;
}

static int finish_step(http1 *state, llhttp_errno_t code) {
  if (code == HPE_OK || code == HPE_PAUSED_UPGRADE)
    return 0;
  state->failed = 1;
  if (state->error.code)
    return state->error.code;
  const char *reason = llhttp_get_error_reason(&state->parser);
  if (code == HPE_INVALID_METHOD)
    reason = "llhttp 9.4.3 does not support this HTTP/1 method";
  if (code == HPE_PAUSED_H2_UPGRADE)
    reason = "binary HTTP/2 requires an HTTP/2 session";
  return http_fail(&state->error, code,
                   reason ? reason : llhttp_errno_name(code));
}

http_step scoop_http1_receive(uint64_t handle, const uint8_t *data,
                              int64_t length) {
  http1 *state = (http1 *)(uintptr_t)handle;
  if (state->failed)
    return (http_step){0, state->error.code};
  if (state->ended || state->switched)
    return (http_step){
        0, http_fail(&state->error, -2, "HTTP/1 input is ended or switched")};
  state->error.code = 0;
  llhttp_errno_t code =
      llhttp_execute(&state->parser, (const char *)data, (size_t)length);
  int64_t consumed = length;
  if (code != HPE_OK)
    consumed = llhttp_get_error_pos(&state->parser) - (const char *)data;
  return (http_step){consumed, finish_step(state, code)};
}

int32_t scoop_http1_end(uint64_t handle) {
  http1 *state = (http1 *)(uintptr_t)handle;
  if (state->failed)
    return state->error.code;
  if (state->ended || state->switched)
    return 0;
  state->ended = 1;
  state->error.code = 0;
  return finish_step(state, llhttp_finish(&state->parser));
}

int32_t scoop_http1_resume(uint64_t handle) {
  http1 *state = (http1 *)(uintptr_t)handle;
  if (!state->server || !state->switched || state->failed)
    return http_fail(&state->error, -2,
                     "only a declined request upgrade can resume HTTP/1");
  llhttp_resume_after_upgrade(&state->parser);
  state->switched = 0;
  return 0;
}

int32_t scoop_http1_switched(uint64_t handle) {
  return ((http1 *)(uintptr_t)handle)->switched;
}
const char *scoop_http1_error(uint64_t handle) {
  return ((http1 *)(uintptr_t)handle)->error.text;
}
uint64_t scoop_http1_next(uint64_t handle) {
  return (uint64_t)(uintptr_t)http_events_pop(
      &((http1 *)(uintptr_t)handle)->events);
}
