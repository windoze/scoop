#include "http2.h"
#include <stdlib.h>
#include <string.h>

int http2_result(http2 *h, int code) {
  if (code < 0)
    http_fail(&h->error, code, nghttp2_strerror(code));
  return code;
}

uint64_t scoop_http2_new(int32_t server, http2_config config,
                         http2_settings settings) {
  http2 *h = calloc(1, sizeof(*h));
  nghttp2_session_callbacks *callbacks = NULL;
  nghttp2_option *options = NULL;
  if (!h)
    return 0;
  h->config = config;
  h->server = server;
  if (nghttp2_session_callbacks_new(&callbacks) || nghttp2_option_new(&options))
    goto fail;
  http2_header_callbacks(callbacks);
  http2_event_callbacks(callbacks);
  nghttp2_option_set_no_auto_window_update(options, 1);
  nghttp2_option_set_no_rfc9113_leading_and_trailing_ws_validation(
      options, config.edge_whitespace);
  nghttp2_option_set_max_continuations(options, (size_t)config.continuations);
  nghttp2_option_set_max_send_header_block_length(
      options, (size_t)config.send_header_bytes);
  int code =
      server ? nghttp2_session_server_new2(&h->session, callbacks, h, options)
             : nghttp2_session_client_new2(&h->session, callbacks, h, options);
  if (code)
    goto fail;
  nghttp2_settings_entry entries[] = {
      {NGHTTP2_SETTINGS_HEADER_TABLE_SIZE, settings.table_size},
      {NGHTTP2_SETTINGS_MAX_CONCURRENT_STREAMS, settings.concurrent_streams},
      {NGHTTP2_SETTINGS_INITIAL_WINDOW_SIZE, settings.initial_window},
      {NGHTTP2_SETTINGS_MAX_FRAME_SIZE, settings.frame_size},
      {NGHTTP2_SETTINGS_MAX_HEADER_LIST_SIZE, settings.header_size},
      {NGHTTP2_SETTINGS_ENABLE_PUSH, 0}};
  if (nghttp2_submit_settings(h->session, NGHTTP2_FLAG_NONE, entries,
                              server ? 5 : 6))
    goto fail;
  nghttp2_option_del(options);
  nghttp2_session_callbacks_del(callbacks);
  return (uint64_t)(uintptr_t)h;
fail:
  nghttp2_session_del(h->session);
  nghttp2_option_del(options);
  nghttp2_session_callbacks_del(callbacks);
  free(h);
  return 0;
}

void scoop_http2_free(uint64_t handle) {
  http2 *h = (http2 *)(uintptr_t)handle;
  nghttp2_session_del(h->session);
  while (h->streams)
    http2_stream_free(h, h->streams);
  http_events_clear(&h->events);
  http_event_free(h->headers);
  http_bytes_clear(&h->output);
  free(h);
}

const char *scoop_http2_error(uint64_t handle) {
  return ((http2 *)(uintptr_t)handle)->error.text;
}

http_step scoop_http2_receive(uint64_t handle, const uint8_t *data,
                              int64_t size) {
  http2 *h = (http2 *)(uintptr_t)handle;
  if (h->ended || h->failed)
    return (http_step){0, http2_result(h, NGHTTP2_ERR_INVALID_STATE)};
  nghttp2_ssize count =
      nghttp2_session_mem_recv2(h->session, data, (size_t)size);
  if (count < 0) {
    h->failed = 1;
    return (http_step){0, http2_result(h, (int)count)};
  }
  return (http_step){count, 0};
}

http_step scoop_http2_output(uint64_t handle, uint8_t *data, int64_t capacity) {
  http2 *h = (http2 *)(uintptr_t)handle;
  if (h->failed)
    return (http_step){0, http2_result(h, NGHTTP2_ERR_INVALID_STATE)};
  if (h->output_offset == h->output.size) {
    h->output_offset = h->output.size = 0;
    const uint8_t *bytes;
    nghttp2_ssize size = nghttp2_session_mem_send2(h->session, &bytes);
    if (size < 0) {
      h->failed = 1;
      return (http_step){0, http2_result(h, (int)size)};
    }
    if (!http_bytes_append(&h->output, bytes, (size_t)size)) {
      h->failed = 1;
      return (http_step){0, http2_result(h, NGHTTP2_ERR_NOMEM)};
    }
  }
  size_t count = h->output.size - h->output_offset;
  if (count > (size_t)capacity)
    count = (size_t)capacity;
  if (count)
    memcpy(data, h->output.data + h->output_offset, count);
  h->output_offset += count;
  return (http_step){(int64_t)count, 0};
}

int32_t scoop_http2_wants(uint64_t handle, int32_t write) {
  http2 *h = (http2 *)(uintptr_t)handle;
  if (h->failed)
    return 0;
  return write ? h->output_offset != h->output.size ||
                     nghttp2_session_want_write(h->session)
               : !h->ended && nghttp2_session_want_read(h->session);
}

uint64_t scoop_http2_next(uint64_t handle) {
  return (uint64_t)(uintptr_t)http_events_pop(
      &((http2 *)(uintptr_t)handle)->events);
}

int32_t scoop_http2_end(uint64_t handle) {
  http2 *h = (http2 *)(uintptr_t)handle;
  if (h->ended)
    return 0;
  h->ended = 1;
  if (nghttp2_session_want_read(h->session)) {
    h->failed = 1;
    return http2_result(h, NGHTTP2_ERR_EOF);
  }
  return 0;
}

int32_t scoop_http2_goaway(uint64_t handle, int32_t last, uint32_t code,
                           const uint8_t *data, int64_t size) {
  http2 *h = (http2 *)(uintptr_t)handle;
  if (last < 0)
    last = nghttp2_session_get_last_proc_stream_id(h->session);
  return http2_result(h, nghttp2_submit_goaway(h->session, NGHTTP2_FLAG_NONE,
                                               last, code, data, (size_t)size));
}
