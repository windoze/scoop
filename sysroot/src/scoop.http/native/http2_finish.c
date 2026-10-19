#include "http2.h"
#include <stdlib.h>

int http2_queue_trailers(http2 *h, http2_stream *s) {
  nghttp2_nv *nva = s->trailers.size ? http2_nv(&s->trailers) : NULL;
  if (s->trailers.size && !nva)
    return NGHTTP2_ERR_NOMEM;
  int code = nghttp2_submit_trailer(h->session, s->id, nva, s->trailers.size);
  free(nva);
  if (!code) {
    s->pending_headers++;
    s->end_headers_queued = 1;
    http_fields_clear(&s->trailers);
  }
  return code;
}

int http2_finish_ready(http2 *h, http2_stream *s) {
  if (s->closed || s->reset || !s->send_eof || !s->send_end_pending ||
      s->end_headers_queued || s->pending_headers ||
      s->body_offset != s->body.size)
    return 0;
  /* Empty DATA providers are not polled while either send window is zero. */
  return http2_queue_trailers(h, s);
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
  s->send_end_pending = 1;
  int code = http2_finish_ready(h, s);
  if (code || s->end_headers_queued)
    return http2_result(h, code);
  code = nghttp2_session_resume_data(h->session, id);
  return code == NGHTTP2_ERR_INVALID_ARGUMENT ? 0 : http2_result(h, code);
}
