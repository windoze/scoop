#ifndef SCOOP_HTTP1_H
#define SCOOP_HTTP1_H
#include "http.h"
#include <llhttp.h>

typedef struct {
  int64_t max_header_bytes, max_field_bytes, max_fields, max_start_line_bytes;
  uint32_t flags;
  int32_t after_close;
} http1_config;

typedef struct http1_method {
  llhttp_method_t method;
  struct http1_method *next;
} http1_method;

typedef struct {
  llhttp_t parser;
  llhttp_settings_t callbacks;
  http1_config config;
  http_error error;
  http_events events;
  http_fields fields;
  http_bytes method, target, reason, protocol, field_name, field_value;
  size_t header_bytes, start_line_bytes;
  int64_t message_starts;
  http1_method *first_method, *last_method;
  int server, failed, ended, switched;
} http1;

void http1_callbacks(http1 *state);
int http1_fail(http1 *state, int code, const char *message);

#endif
