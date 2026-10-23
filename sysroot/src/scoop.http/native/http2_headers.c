#include "http2.h"
#include <string.h>

static int begin_headers(nghttp2_session *session, const nghttp2_frame *frame,
                         void *user_data) {
  (void)session;
  http2 *h = user_data;
  if (frame->hd.type != NGHTTP2_HEADERS)
    return NGHTTP2_ERR_TEMPORAL_CALLBACK_FAILURE;
  http2_stream *s = http2_find(h, frame->hd.stream_id);
  if (!s) {
    s = http2_stream_new(h, frame->hd.stream_id);
    if (!s)
      return NGHTTP2_ERR_CALLBACK_FAILURE;
  }
  http_event_free(h->headers);
  int kind = s->received_final ? HTTP_EVENT_TRAILERS
             : h->server       ? HTTP_EVENT_REQUEST
                               : HTTP_EVENT_RESPONSE;
  h->headers = http_event_new(kind);
  if (!h->headers)
    return NGHTTP2_ERR_CALLBACK_FAILURE;
  h->headers->info.version = 20;
  h->headers->info.stream_id = frame->hd.stream_id;
  h->header_bytes = h->header_count = 0;
  return kind == HTTP_EVENT_REQUEST
             ? http2_push_simple(h, HTTP_EVENT_REQUEST_HEAD_START,
                                 frame->hd.stream_id, 0)
             : 0;
}

static int header_limit(http2 *h, int32_t id, size_t name, size_t value) {
  size_t max = (size_t)h->config.field_bytes;
  if (name > max || value > max - name ||
      name + value > (size_t)h->config.header_bytes - h->header_bytes ||
      h->header_count >= (size_t)h->config.fields) {
    int code =
        nghttp2_submit_rst_stream(h->session, 0, id, NGHTTP2_ENHANCE_YOUR_CALM);
    return code ? NGHTTP2_ERR_CALLBACK_FAILURE
                : NGHTTP2_ERR_TEMPORAL_CALLBACK_FAILURE;
  }
  h->header_bytes += name + value;
  ++h->header_count;
  return 0;
}

static int copy_field(http_fields *fields, const uint8_t *name, size_t namelen,
                      const uint8_t *value, size_t valuelen, uint8_t flags) {
  http_bytes n = {0}, v = {0};
  if (!http_bytes_append(&n, name, namelen) ||
      !http_bytes_append(&v, value, valuelen) ||
      !http_fields_add(fields, &n, &v,
                       (flags & NGHTTP2_NV_FLAG_NO_INDEX) != 0)) {
    http_bytes_clear(&n);
    http_bytes_clear(&v);
    return NGHTTP2_ERR_CALLBACK_FAILURE;
  }
  return 0;
}

static int header(nghttp2_session *session, const nghttp2_frame *frame,
                  const uint8_t *name, size_t namelen, const uint8_t *value,
                  size_t valuelen, uint8_t flags, void *user_data) {
  (void)session;
  http2 *h = user_data;
  int code = header_limit(h, frame->hd.stream_id, namelen, valuelen);
  if (code)
    return code;
  if (name[0] != ':')
    return copy_field(&h->headers->fields, name, namelen, value, valuelen,
                      flags);
  int slot;
  if (namelen == 7 && !memcmp(name, ":method", 7)) {
    slot = 0;
    http2_stream *s = http2_find(h, frame->hd.stream_id);
    s->head_request = valuelen == 4 && !memcmp(value, "HEAD", 4);
    s->connect_request = valuelen == 7 && !memcmp(value, "CONNECT", 7);
  } else if (namelen == 7 && !memcmp(name, ":scheme", 7))
    slot = 1;
  else if (namelen == 10 && !memcmp(name, ":authority", 10))
    slot = 2;
  else if (namelen == 5 && !memcmp(name, ":path", 5))
    slot = 3;
  else if (namelen == 7 && !memcmp(name, ":status", 7)) {
    h->headers->info.status =
        (value[0] - '0') * 100 + (value[1] - '0') * 10 + value[2] - '0';
    if (flags & NGHTTP2_NV_FLAG_NO_INDEX)
      h->headers->info.flags |= 16;
    return 0;
  } else
    return NGHTTP2_ERR_TEMPORAL_CALLBACK_FAILURE;
  if (flags & NGHTTP2_NV_FLAG_NO_INDEX)
    h->headers->info.flags |= 1U << slot;
  return http_bytes_append(&h->headers->bytes[slot], value, valuelen)
             ? 0
             : NGHTTP2_ERR_CALLBACK_FAILURE;
}

static int invalid_header(nghttp2_session *session, const nghttp2_frame *frame,
                          const uint8_t *name, size_t namelen,
                          const uint8_t *value, size_t valuelen, uint8_t flags,
                          void *user_data) {
  (void)session;
  http2 *h = user_data;
  int code = header_limit(h, frame->hd.stream_id, namelen, valuelen);
  if (code)
    return code;
  if (h->config.reject_invalid)
    return NGHTTP2_ERR_TEMPORAL_CALLBACK_FAILURE;
  http_event *event = http_event_new(HTTP_EVENT_IGNORED);
  if (!event)
    return NGHTTP2_ERR_CALLBACK_FAILURE;
  event->info.stream_id = frame->hd.stream_id;
  code = copy_field(&event->fields, name, namelen, value, valuelen, flags);
  if (code)
    http_event_free(event);
  else
    http_events_push(&h->events, event);
  return code;
}

void http2_header_callbacks(nghttp2_session_callbacks *callbacks) {
  nghttp2_session_callbacks_set_on_begin_headers_callback(callbacks,
                                                          begin_headers);
  nghttp2_session_callbacks_set_on_header_callback(callbacks, header);
  nghttp2_session_callbacks_set_on_invalid_header_callback(callbacks,
                                                           invalid_header);
}
