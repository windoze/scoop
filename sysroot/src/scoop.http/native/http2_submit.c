#include "http2.h"
#include <stdlib.h>
#include <string.h>

uint64_t scoop_http2_fields_new(void) {
  return (uint64_t)(uintptr_t)calloc(1, sizeof(http_fields));
}

void scoop_http2_fields_free(uint64_t handle) {
  http_fields *fields = (http_fields *)(uintptr_t)handle;
  http_fields_clear(fields);
  free(fields);
}

int32_t scoop_http2_field_add(uint64_t handle, const uint8_t *name,
                              int64_t name_size, const uint8_t *value,
                              int64_t value_size, int32_t never_index) {
  if (!nghttp2_check_header_name(name, (size_t)name_size) ||
      !nghttp2_check_header_value_rfc9113(value, (size_t)value_size))
    return NGHTTP2_ERR_INVALID_ARGUMENT;
  if ((name_size == 7 && !memcmp(name, ":method", 7) &&
       !nghttp2_check_method(value, (size_t)value_size)) ||
      (name_size == 5 && !memcmp(name, ":path", 5) &&
       !nghttp2_check_path(value, (size_t)value_size)) ||
      (name_size == 10 && !memcmp(name, ":authority", 10) &&
       !nghttp2_check_authority(value, (size_t)value_size)))
    return NGHTTP2_ERR_INVALID_ARGUMENT;
  http_bytes n = {0}, v = {0};
  if (!http_bytes_append(&n, name, (size_t)name_size) ||
      !http_bytes_append(&v, value, (size_t)value_size) ||
      !http_fields_add((http_fields *)(uintptr_t)handle, &n, &v, never_index)) {
    http_bytes_clear(&n);
    http_bytes_clear(&v);
    return NGHTTP2_ERR_NOMEM;
  }
  return 0;
}

nghttp2_nv *http2_nv(const http_fields *fields) {
  nghttp2_nv *nva = calloc(fields->size ? fields->size : 1, sizeof(*nva));
  if (!nva)
    return NULL;
  for (size_t i = 0; i < fields->size; ++i) {
    const http_field *f = &fields->items[i];
    nva[i] =
        (nghttp2_nv){f->name.data, f->value.data, f->name.size, f->value.size,
                     f->never_index ? NGHTTP2_NV_FLAG_NO_INDEX : 0};
  }
  return nva;
}

int32_t scoop_http2_request(uint64_t handle, uint64_t fields, int32_t end,
                            int64_t length) {
  http2 *h = (http2 *)(uintptr_t)handle;
  if (h->server || h->failed || h->ended)
    return http2_result(h, NGHTTP2_ERR_INVALID_STATE);
  if (end && length > 0)
    return http2_result(h, NGHTTP2_ERR_INVALID_ARGUMENT);
  http_fields *headers = (http_fields *)(uintptr_t)fields;
  nghttp2_nv *nva = http2_nv(headers);
  if (!nva)
    return http2_result(h, NGHTTP2_ERR_NOMEM);
  http2_stream *s = http2_stream_new(h, 0);
  if (!s) {
    free(nva);
    return http2_result(h, NGHTTP2_ERR_NOMEM);
  }
  /* The adapter always places :method first. */
  s->head_request = nva[0].valuelen == 4 && !memcmp(nva[0].value, "HEAD", 4);
  s->connect_request =
      nva[0].valuelen == 7 && !memcmp(nva[0].value, "CONNECT", 7);
  s->send_open = !end;
  s->remaining = length;
  nghttp2_data_provider2 provider = http2_provider(s);
  int32_t id = nghttp2_submit_request2(h->session, NULL, nva, headers->size,
                                       end ? NULL : &provider, s);
  free(nva);
  if (id < 0) {
    http2_stream_free(h, s);
    return http2_result(h, id);
  }
  s->id = id;
  s->pending_headers = 1;
  s->send_end_pending = end;
  return id;
}

int32_t scoop_http2_response(uint64_t handle, int32_t id, uint64_t fields,
                             int32_t status, int32_t end, int64_t length) {
  http2 *h = (http2 *)(uintptr_t)handle;
  http2_stream *s = http2_find(h, id);
  if (!h->server || !s || s->closed || s->reset || s->sent_final || h->failed)
    return http2_result(h, NGHTTP2_ERR_INVALID_STATE);
  int informational = status < 200;
  int metadata = s->head_request || status == 304;
  int no_body = metadata || status == 204;
  if ((informational && (end || length >= 0)) || (no_body && !end) ||
      (status == 204 && length >= 0) ||
      (s->connect_request && status >= 200 && status < 300 && length >= 0) ||
      (end && length > 0 && !metadata))
    return http2_result(h, NGHTTP2_ERR_INVALID_ARGUMENT);
  http_fields *headers = (http_fields *)(uintptr_t)fields;
  nghttp2_nv *nva = http2_nv(headers);
  if (!nva)
    return http2_result(h, NGHTTP2_ERR_NOMEM);
  nghttp2_data_provider2 provider = http2_provider(s);
  int code = informational
                 ? nghttp2_submit_headers(h->session, 0, id, NULL, nva,
                                          headers->size, NULL)
                 : nghttp2_submit_response2(h->session, id, nva, headers->size,
                                            end ? NULL : &provider);
  free(nva);
  if (!code)
    s->pending_headers++;
  if (!code && !informational) {
    s->sent_final = 1;
    s->send_open = !end;
    s->send_end_pending = end;
    s->remaining = metadata ? -1 : length;
  }
  return http2_result(h, code);
}
