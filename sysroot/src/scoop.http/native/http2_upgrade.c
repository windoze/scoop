#include "http2.h"

int32_t scoop_http2_upgrade(uint64_t handle, const uint8_t *payload,
                            int64_t size, int32_t head) {
  http2 *h = (http2 *)(uintptr_t)handle;
  if (h->streams || h->failed || h->ended)
    return http2_result(h, NGHTTP2_ERR_INVALID_STATE);
  http2_stream *s = http2_stream_new(h, 1);
  if (!s)
    return http2_result(h, NGHTTP2_ERR_NOMEM);
  s->head_request = head;
  s->received_final = h->server;
  s->received_trailers = h->server;
  int code =
      nghttp2_session_upgrade2(h->session, payload, (size_t)size, head, s);
  if (code)
    http2_stream_free(h, s);
  return http2_result(h, code);
}
