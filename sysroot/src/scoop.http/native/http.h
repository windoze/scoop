#ifndef SCOOP_HTTP_H
#define SCOOP_HTTP_H

#include <stddef.h>
#include <stdint.h>

typedef struct {
  uint8_t *data;
  size_t size, capacity;
} http_bytes;

typedef struct {
  http_bytes name, value;
  int32_t never_index;
} http_field;

typedef struct {
  http_field *items;
  size_t size, capacity;
} http_fields;

enum {
  HTTP_EVENT_REQUEST = 1,
  HTTP_EVENT_RESPONSE,
  HTTP_EVENT_DATA,
  HTTP_EVENT_TRAILERS,
  HTTP_EVENT_END,
  HTTP_EVENT_SWITCH,
  HTTP_EVENT_STREAM_CLOSE,
  HTTP_EVENT_GOAWAY,
  HTTP_EVENT_SETTINGS,
  HTTP_EVENT_IGNORED,
  HTTP_EVENT_ERROR
};

typedef struct {
  int32_t kind, version, stream_id, status;
  uint32_t flags;
  int32_t code;
  int64_t amount;
} http_event_info;

typedef struct http_event {
  http_event_info info;
  http_bytes bytes[4];
  http_fields fields;
  struct http_event *next;
} http_event;

typedef struct {
  http_event *first, *last;
} http_events;
typedef struct {
  int64_t count;
  int32_t code;
} http_step;
typedef struct {
  int32_t code;
  char text[256];
} http_error;
typedef struct {
  const uint8_t *data;
  int64_t size;
} http_byte_view;
typedef struct {
  const uint8_t *name;
  int64_t name_size;
  const uint8_t *value;
  int64_t value_size;
  int32_t never_index;
} http_field_view;

int http_bytes_append(http_bytes *bytes, const void *data, size_t size);
void http_bytes_clear(http_bytes *bytes);
int http_fields_add(http_fields *fields, http_bytes *name, http_bytes *value,
                    int32_t never_index);
void http_fields_clear(http_fields *fields);
http_event *http_event_new(int32_t kind);
void http_event_free(http_event *event);
void http_events_push(http_events *events, http_event *event);
http_event *http_events_pop(http_events *events);
void http_events_clear(http_events *events);
int http_fail(http_error *error, int code, const char *message);

#endif
