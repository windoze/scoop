#include "http1.h"
#include <stdlib.h>
#include <string.h>

static int begin(llhttp_t *parser) {
  http1 *s = parser->data;
  s->header_bytes = s->start_line_bytes = 0;
  http_bytes_clear(&s->protocol);
  if (!s->server) {
    if (!s->first_method)
      return http1_fail(s, -2,
                        "HTTP/1 response has no corresponding request method");
    parser->method = s->first_method->method;
  }
  return 0;
}

static int line_part(llhttp_t *p, http_bytes *output, const char *data,
                     size_t length) {
  http1 *s = p->data;
  if (length > (size_t)s->config.max_start_line_bytes - s->start_line_bytes)
    return http1_fail(s, -3, "HTTP/1 start line exceeds its byte limit");
  s->start_line_bytes += length;
  if (output && !http_bytes_append(output, data, length))
    return http1_fail(s, -1, "cannot allocate HTTP/1 start line");
  return 0;
}

static int method(llhttp_t *p, const char *data, size_t length) {
  return line_part(p, &((http1 *)p->data)->method, data, length);
}
static int target(llhttp_t *p, const char *data, size_t length) {
  return line_part(p, &((http1 *)p->data)->target, data, length);
}
static int reason(llhttp_t *p, const char *data, size_t length) {
  return line_part(p, &((http1 *)p->data)->reason, data, length);
}
static int protocol(llhttp_t *p, const char *data, size_t length) {
  return line_part(p, &((http1 *)p->data)->protocol, data, length);
}
static int version(llhttp_t *p, const char *data, size_t length) {
  return line_part(p, NULL, data, length);
}

static int field_part(llhttp_t *p, http_bytes *output, const char *data,
                      size_t length) {
  http1 *s = p->data;
  size_t field_size = s->field_name.size + s->field_value.size;
  if (length > (size_t)s->config.max_header_bytes - s->header_bytes ||
      length > (size_t)s->config.max_field_bytes - field_size)
    return http1_fail(s, -3, "HTTP/1 field bytes exceed the configured limit");
  s->header_bytes += length;
  if (!http_bytes_append(output, data, length))
    return http1_fail(s, -1, "cannot allocate HTTP/1 field");
  return 0;
}

static int field_name(llhttp_t *p, const char *data, size_t length) {
  return field_part(p, &((http1 *)p->data)->field_name, data, length);
}
static int field_value(llhttp_t *p, const char *data, size_t length) {
  return field_part(p, &((http1 *)p->data)->field_value, data, length);
}

static int field_complete(llhttp_t *p) {
  http1 *s = p->data;
  if (s->fields.size >= (size_t)s->config.max_fields)
    return http1_fail(s, -3, "HTTP/1 field count exceeds the configured limit");
  if (!http_fields_add(&s->fields, &s->field_name, &s->field_value, 0))
    return http1_fail(s, -1, "cannot allocate HTTP/1 fields");
  return 0;
}

static int headers(llhttp_t *p) {
  http1 *s = p->data;
  if (p->http_major != 1 || p->http_minor > 1 || s->protocol.size != 4 ||
      memcmp(s->protocol.data, "HTTP", 4))
    return http1_fail(
        s, -4, "only HTTP/1.0 and HTTP/1.1 textual messages are supported");
  http_event *event =
      http_event_new(s->server ? HTTP_EVENT_REQUEST : HTTP_EVENT_RESPONSE);
  if (!event)
    return http1_fail(s, -1, "cannot allocate HTTP/1 header event");
  event->info.version = 10 + p->http_minor;
  event->info.status = p->status_code;
  event->fields = s->fields;
  s->fields = (http_fields){0};
  s->header_bytes = 0;
  if (s->server) {
    event->bytes[0] = s->method;
    event->bytes[1] = s->target;
    s->method = s->target = (http_bytes){0};
  } else {
    event->bytes[0] = s->reason;
    s->reason = (http_bytes){0};
  }
  http_events_push(&s->events, event);
  if (!s->server && s->first_method->method == HTTP_CONNECT &&
      p->status_code >= 200 && p->status_code < 300)
    return 2;
  if (!s->server && s->first_method->method == HTTP_HEAD)
    return 1;
  return 0;
}

static int body(llhttp_t *p, const char *data, size_t length) {
  http1 *s = p->data;
  http_event *event = http_event_new(HTTP_EVENT_DATA);
  if (!event)
    return http1_fail(s, -1, "cannot allocate HTTP/1 body event");
  if (!http_bytes_append(&event->bytes[0], data, length)) {
    http_event_free(event);
    return http1_fail(s, -1, "cannot allocate HTTP/1 body bytes");
  }
  http_events_push(&s->events, event);
  return 0;
}

static int complete(llhttp_t *p) {
  http1 *s = p->data;
  http_event *trailers = http_event_new(HTTP_EVENT_TRAILERS);
  http_event *end = http_event_new(HTTP_EVENT_END);
  http_event *upgrade = p->upgrade ? http_event_new(HTTP_EVENT_SWITCH) : NULL;
  if (!trailers || !end || (p->upgrade && !upgrade)) {
    http_event_free(trailers);
    http_event_free(end);
    http_event_free(upgrade);
    return http1_fail(s, -1, "cannot allocate HTTP/1 completion events");
  }
  trailers->fields = s->fields;
  s->fields = (http_fields){0};
  end->info.flags = (uint32_t)llhttp_should_keep_alive(p);
  http_events_push(&s->events, trailers);
  http_events_push(&s->events, end);
  if (upgrade) {
    s->switched = 1;
    http_events_push(&s->events, upgrade);
  }
  if (!s->server && (p->status_code >= 200 || p->status_code == 101)) {
    http1_method *method = s->first_method;
    s->first_method = method->next;
    if (!s->first_method)
      s->last_method = NULL;
    free(method);
  }
  return 0;
}

void http1_callbacks(http1 *s) {
  llhttp_settings_init(&s->callbacks);
  s->callbacks.on_message_begin = begin;
  s->callbacks.on_method = method;
  s->callbacks.on_url = target;
  s->callbacks.on_status = reason;
  s->callbacks.on_protocol = protocol;
  s->callbacks.on_version = version;
  s->callbacks.on_header_field = field_name;
  s->callbacks.on_header_value = field_value;
  s->callbacks.on_header_value_complete = field_complete;
  s->callbacks.on_headers_complete = headers;
  s->callbacks.on_body = body;
  s->callbacks.on_message_complete = complete;
}
