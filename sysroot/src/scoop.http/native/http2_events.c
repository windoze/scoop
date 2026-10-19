#include "http2.h"
#include <string.h>

int http2_push_simple(http2 *h, int kind, int32_t id, uint32_t flags) {
  http_event *event = http_event_new(kind);
  if (!event)
    return NGHTTP2_ERR_CALLBACK_FAILURE;
  event->info.stream_id = id;
  event->info.flags = flags;
  http_events_push(&h->events, event);
  return 0;
}

static int data_chunk(nghttp2_session *session, uint8_t flags, int32_t id,
                      const uint8_t *data, size_t size, void *user_data) {
  (void)flags;
  http2 *h = user_data;
  http2_stream *s = http2_find(h, id);
  if (!s)
    return NGHTTP2_ERR_CALLBACK_FAILURE;
  if (s->reset)
    return nghttp2_session_consume_connection(session, size);
  http_event *event = http_event_new(HTTP_EVENT_DATA);
  if (!event)
    return NGHTTP2_ERR_CALLBACK_FAILURE;
  event->info.stream_id = id;
  if (!http_bytes_append(&event->bytes[0], data, size)) {
    http_event_free(event);
    return NGHTTP2_ERR_CALLBACK_FAILURE;
  }
  s->unconsumed += (int64_t)size;
  http_events_push(&h->events, event);
  return 0;
}

static int goaway(http2 *h, const nghttp2_frame *frame, int local) {
  http_event *event = http_event_new(HTTP_EVENT_GOAWAY);
  if (!event)
    return NGHTTP2_ERR_CALLBACK_FAILURE;
  event->info.stream_id = frame->goaway.last_stream_id;
  event->info.flags = frame->goaway.error_code;
  event->info.status = local;
  if (!http_bytes_append(&event->bytes[0], frame->goaway.opaque_data,
                         frame->goaway.opaque_data_len)) {
    http_event_free(event);
    return NGHTTP2_ERR_CALLBACK_FAILURE;
  }
  http_events_push(&h->events, event);
  return 0;
}

static int settings(http2 *h, const nghttp2_frame *frame) {
  http_event *event = http_event_new(HTTP_EVENT_SETTINGS);
  if (!event)
    return NGHTTP2_ERR_CALLBACK_FAILURE;
  event->info.flags = (frame->hd.flags & NGHTTP2_FLAG_ACK) != 0;
  for (size_t i = 0; i < frame->settings.niv; ++i) {
    const nghttp2_settings_entry *entry = &frame->settings.iv[i];
    uint32_t id = (uint32_t)entry->settings_id, value = entry->value;
    uint8_t bytes[] = {(uint8_t)(id >> 8),     (uint8_t)id,
                       (uint8_t)(value >> 24), (uint8_t)(value >> 16),
                       (uint8_t)(value >> 8),  (uint8_t)value};
    if (!http_bytes_append(&event->bytes[0], bytes, sizeof(bytes))) {
      http_event_free(event);
      return NGHTTP2_ERR_CALLBACK_FAILURE;
    }
  }
  http_events_push(&h->events, event);
  return 0;
}

static int frame_recv(nghttp2_session *session, const nghttp2_frame *frame,
                      void *user_data) {
  (void)session;
  http2 *h = user_data;
  if (frame->hd.type == NGHTTP2_GOAWAY)
    return goaway(h, frame, 0);
  if (frame->hd.type == NGHTTP2_SETTINGS)
    return settings(h, frame);
  if (frame->hd.type != NGHTTP2_HEADERS && frame->hd.type != NGHTTP2_DATA)
    return 0;
  http2_stream *s = http2_find(h, frame->hd.stream_id);
  if (!s)
    return NGHTTP2_ERR_CALLBACK_FAILURE;
  if (frame->hd.type == NGHTTP2_HEADERS) {
    http_event *headers = h->headers;
    h->headers = NULL;
    if (!headers)
      return NGHTTP2_ERR_CALLBACK_FAILURE;
    if (headers->info.kind == HTTP_EVENT_TRAILERS)
      s->received_trailers = 1;
    else if (headers->info.kind == HTTP_EVENT_REQUEST ||
             headers->info.status >= 200)
      s->received_final = 1;
    http_events_push(&h->events, headers);
  }
  if (frame->hd.flags & NGHTTP2_FLAG_END_STREAM) {
    if (!s->received_trailers) {
      int code = http2_push_simple(h, HTTP_EVENT_TRAILERS, s->id, 0);
      if (code)
        return code;
      s->received_trailers = 1;
    }
    return http2_push_simple(h, HTTP_EVENT_END, s->id, 0);
  }
  return 0;
}

static int stream_close(nghttp2_session *session, int32_t id, uint32_t code,
                        void *user_data) {
  (void)session;
  http2 *h = user_data;
  http2_stream *s = http2_find(h, id);
  if (h->headers && h->headers->info.stream_id == id) {
    http_event_free(h->headers);
    h->headers = NULL;
  }
  if (s) {
    s->closed = 1;
    http_bytes_clear(&s->body);
    s->body_offset = 0;
    http_fields_clear(&s->trailers);
    http2_stream_maybe_free(h, s);
  }
  return http2_push_simple(h, HTTP_EVENT_STREAM_CLOSE, id, code);
}

static int frame_sent(nghttp2_session *session, const nghttp2_frame *frame,
                      void *user_data) {
  (void)session;
  http2 *h = user_data;
  http2_stream *s = http2_find(h, frame->hd.stream_id);
  if (s) {
    if (frame->hd.type == NGHTTP2_HEADERS && s->pending_headers)
      s->pending_headers--;
    if ((frame->hd.type == NGHTTP2_HEADERS || frame->hd.type == NGHTTP2_DATA) &&
        (frame->hd.flags & NGHTTP2_FLAG_END_STREAM))
      s->send_end_pending = 0;
  }
  return frame->hd.type == NGHTTP2_GOAWAY ? goaway(user_data, frame, 1) : 0;
}

static int frame_sending(nghttp2_session *session, const nghttp2_frame *frame,
                         void *user_data) {
  (void)session;
  http2 *h = user_data;
  h->output_stream_id = frame->hd.stream_id;
  h->output_kind = frame->hd.type == NGHTTP2_HEADERS ? HTTP2_OUTPUT_HEADERS
                                                     : HTTP2_OUTPUT_CONTROL;
  return 0;
}

static int frame_not_sent(nghttp2_session *session, const nghttp2_frame *frame,
                          int code, void *user_data) {
  (void)session;
  http2 *h = user_data;
  http_event *event = http_event_new(HTTP_EVENT_ERROR);
  if (!event)
    return NGHTTP2_ERR_CALLBACK_FAILURE;
  event->info.stream_id = frame->hd.stream_id;
  event->info.code = code;
  const char *message = nghttp2_strerror(code);
  if (!http_bytes_append(&event->bytes[0], message, strlen(message))) {
    http_event_free(event);
    return NGHTTP2_ERR_CALLBACK_FAILURE;
  }
  http_events_push(&h->events, event);
  return 0;
}

void http2_event_callbacks(nghttp2_session_callbacks *callbacks) {
  nghttp2_session_callbacks_set_on_data_chunk_recv_callback(callbacks,
                                                            data_chunk);
  nghttp2_session_callbacks_set_on_frame_recv_callback(callbacks, frame_recv);
  nghttp2_session_callbacks_set_on_stream_close_callback(callbacks,
                                                         stream_close);
  nghttp2_session_callbacks_set_on_frame_send_callback(callbacks, frame_sent);
  nghttp2_session_callbacks_set_before_frame_send_callback(callbacks,
                                                           frame_sending);
  nghttp2_session_callbacks_set_on_frame_not_send_callback(callbacks,
                                                           frame_not_sent);
}
