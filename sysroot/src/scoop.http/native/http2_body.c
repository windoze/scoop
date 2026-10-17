#include "http2.h"
#include <stdlib.h>
#include <string.h>

http2_stream *http2_find(http2 *h, int32_t id) {
  for (http2_stream *s = h->streams; s; s = s->next)
    if (s->id == id)
      return s;
  return NULL;
}

http2_stream *http2_stream_new(http2 *h, int32_t id) {
  http2_stream *s = calloc(1, sizeof(*s));
  if (!s)
    return NULL;
  s->id = id;
  s->remaining = -1;
  s->next = h->streams;
  h->streams = s;
  return s;
}

void http2_stream_free(http2 *h, http2_stream *s) {
  http2_stream **at = &h->streams;
  while (*at != s)
    at = &(*at)->next;
  *at = s->next;
  http_bytes_clear(&s->body);
  http_fields_clear(&s->trailers);
  free(s);
}

void http2_stream_maybe_free(http2 *h, http2_stream *s) {
  if (s->closed && s->unconsumed == 0)
    http2_stream_free(h, s);
}

static nghttp2_ssize body_read(nghttp2_session *session, int32_t stream_id,
                               uint8_t *buffer, size_t length, uint32_t *flags,
                               nghttp2_data_source *source, void *user_data) {
  (void)user_data;
  http2_stream *s = source->ptr;
  size_t count = s->body.size - s->body_offset;
  if (count > length)
    count = length;
  if (!count && !s->send_eof)
    return NGHTTP2_ERR_DEFERRED;
  if (count)
    memcpy(buffer, s->body.data + s->body_offset, count);
  s->body_offset += count;
  if (s->body_offset == s->body.size) {
    s->body_offset = s->body.size = 0;
    if (s->send_eof) {
      *flags |= NGHTTP2_DATA_FLAG_EOF;
      if (s->trailers.size) {
        nghttp2_nv *nva = http2_nv(&s->trailers);
        if (!nva)
          return NGHTTP2_ERR_CALLBACK_FAILURE;
        int code =
            nghttp2_submit_trailer(session, stream_id, nva, s->trailers.size);
        free(nva);
        if (code)
          return NGHTTP2_ERR_CALLBACK_FAILURE;
        *flags |= NGHTTP2_DATA_FLAG_NO_END_STREAM;
        http_fields_clear(&s->trailers);
      }
    }
  }
  return (nghttp2_ssize)count;
}

nghttp2_data_provider2 http2_provider(http2_stream *s) {
  return (nghttp2_data_provider2){{.ptr = s}, body_read};
}

http_step scoop_http2_write(uint64_t handle, int32_t id, const uint8_t *data,
                            int64_t size) {
  http2 *h = (http2 *)(uintptr_t)handle;
  http2_stream *s = http2_find(h, id);
  if (!s || s->closed || s->reset || !s->send_open || s->send_eof || h->failed)
    return (http_step){0, http2_result(h, NGHTTP2_ERR_INVALID_STATE)};
  if (size == 0)
    return (http_step){0, 0};
  size_t pending = s->body.size - s->body_offset;
  size_t count = (size_t)h->config.pending_body_bytes - pending;
  if (count > (size_t)size)
    count = (size_t)size;
  if (s->remaining >= 0 && (int64_t)count > s->remaining)
    return (http_step){0, http2_result(h, NGHTTP2_ERR_INVALID_ARGUMENT)};
  if (s->body_offset && pending)
    memmove(s->body.data, s->body.data + s->body_offset, pending);
  s->body_offset = 0;
  s->body.size = pending;
  if (!http_bytes_append(&s->body, data, count))
    return (http_step){0, http2_result(h, NGHTTP2_ERR_NOMEM)};
  if (s->remaining >= 0)
    s->remaining -= (int64_t)count;
  /* Not deferred yet is normal before its first DATA callback. */
  int code = nghttp2_session_resume_data(h->session, id);
  if (code != 0 && code != NGHTTP2_ERR_INVALID_ARGUMENT)
    return (http_step){(int64_t)count, http2_result(h, code)};
  return (http_step){(int64_t)count, 0};
}

int32_t scoop_http2_finish(uint64_t handle, int32_t id, uint64_t fields) {
  http2 *h = (http2 *)(uintptr_t)handle;
  http2_stream *s = http2_find(h, id);
  if (!s || s->closed || s->reset || !s->send_open || s->send_eof || h->failed)
    return http2_result(h, NGHTTP2_ERR_INVALID_STATE);
  if (s->remaining > 0)
    return http2_result(h, NGHTTP2_ERR_INVALID_ARGUMENT);
  http_fields *trailers = (http_fields *)(uintptr_t)fields;
  s->trailers = *trailers;
  *trailers = (http_fields){0};
  s->send_eof = 1;
  int code = nghttp2_session_resume_data(h->session, id);
  return code == NGHTTP2_ERR_INVALID_ARGUMENT ? 0 : http2_result(h, code);
}

int32_t scoop_http2_consume(uint64_t handle, int32_t id, int64_t count) {
  http2 *h = (http2 *)(uintptr_t)handle;
  http2_stream *s = http2_find(h, id);
  if (!s || count < 0 || count > s->unconsumed)
    return http2_result(h, NGHTTP2_ERR_INVALID_ARGUMENT);
  int code = s->closed
                 ? nghttp2_session_consume_connection(h->session, (size_t)count)
                 : nghttp2_session_consume(h->session, id, (size_t)count);
  if (!code) {
    s->unconsumed -= count;
    http2_stream_maybe_free(h, s);
  }
  return http2_result(h, code);
}

int32_t scoop_http2_reset(uint64_t handle, int32_t id, uint32_t code) {
  http2 *h = (http2 *)(uintptr_t)handle;
  http2_stream *s = http2_find(h, id);
  if (!s)
    return 0;
  int result =
      s->closed ? 0 : nghttp2_submit_rst_stream(h->session, 0, id, code);
  if (!result)
    result =
        nghttp2_session_consume_connection(h->session, (size_t)s->unconsumed);
  if (!result) {
    s->reset = 1;
    s->unconsumed = 0;
    http_bytes_clear(&s->body);
    s->body_offset = 0;
    http_events kept = {0};
    http_event *event;
    while ((event = http_events_pop(&h->events))) {
      if (event->info.stream_id == id && event->info.kind == HTTP_EVENT_DATA)
        http_event_free(event);
      else
        http_events_push(&kept, event);
    }
    h->events = kept;
    http2_stream_maybe_free(h, s);
  }
  return http2_result(h, result);
}
