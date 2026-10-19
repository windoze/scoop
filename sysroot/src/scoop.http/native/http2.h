#ifndef SCOOP_HTTP2_H
#define SCOOP_HTTP2_H

#include "http.h"
#include <nghttp2/nghttp2.h>

enum { HTTP2_OUTPUT_CONTROL, HTTP2_OUTPUT_HEADERS, HTTP2_OUTPUT_DATA };
typedef struct {
  int64_t count;
  int32_t code, stream_id, kind;
} http2_output;

typedef struct {
  int64_t header_bytes, field_bytes, fields, continuations;
  int64_t send_header_bytes, pending_body_bytes;
  int32_t edge_whitespace, reject_invalid;
} http2_config;
typedef struct {
  uint32_t table_size, concurrent_streams, initial_window, frame_size,
      header_size;
} http2_settings;

typedef struct http2_stream {
  int32_t id;
  int closed, reset, head_request, connect_request;
  int received_final, received_trailers, sent_final, send_open, send_eof;
  int send_end_pending;
  size_t pending_headers;
  int64_t remaining, unconsumed;
  http_bytes body;
  size_t body_offset;
  http_fields trailers;
  struct http2_stream *next;
} http2_stream;

typedef struct {
  nghttp2_session *session;
  http2_config config;
  int server, ended, failed;
  http2_stream *streams;
  http_events events;
  http_event *headers;
  size_t header_bytes, header_count;
  http_bytes output;
  size_t output_offset;
  int32_t output_stream_id, output_kind;
  http_error error;
} http2;

int http2_result(http2 *h, int code);
http2_stream *http2_find(http2 *h, int32_t id);
http2_stream *http2_stream_new(http2 *h, int32_t id);
void http2_stream_free(http2 *h, http2_stream *stream);
void http2_stream_maybe_free(http2 *h, http2_stream *stream);
nghttp2_nv *http2_nv(const http_fields *fields);
nghttp2_data_provider2 http2_provider(http2_stream *stream);
void http2_header_callbacks(nghttp2_session_callbacks *callbacks);
void http2_event_callbacks(nghttp2_session_callbacks *callbacks);
int http2_push_simple(http2 *h, int kind, int32_t id, uint32_t flags);

#endif
