#include "http.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

int http_bytes_append(http_bytes *bytes, const void *data, size_t size) {
  if (size > SIZE_MAX - bytes->size)
    return 0;
  size_t needed = bytes->size + size;
  if (needed > bytes->capacity) {
    size_t capacity =
        bytes->capacity <= SIZE_MAX / 2 ? bytes->capacity * 2 : SIZE_MAX;
    if (capacity < needed)
      capacity = needed;
    void *memory = realloc(bytes->data, capacity);
    if (!memory)
      return 0;
    bytes->data = memory;
    bytes->capacity = capacity;
  }
  if (size)
    memcpy(bytes->data + bytes->size, data, size);
  bytes->size = needed;
  return 1;
}

void http_bytes_clear(http_bytes *bytes) {
  free(bytes->data);
  *bytes = (http_bytes){0};
}

int http_fields_add(http_fields *fields, http_bytes *name, http_bytes *value,
                    int32_t never_index) {
  if (fields->size == fields->capacity) {
    if (fields->capacity > SIZE_MAX / sizeof(http_field) / 2)
      return 0;
    size_t capacity = fields->capacity ? fields->capacity * 2 : 8;
    void *memory = realloc(fields->items, capacity * sizeof(http_field));
    if (!memory)
      return 0;
    fields->items = memory;
    fields->capacity = capacity;
  }
  fields->items[fields->size++] = (http_field){*name, *value, never_index};
  *name = (http_bytes){0};
  *value = (http_bytes){0};
  return 1;
}

void http_fields_clear(http_fields *fields) {
  for (size_t i = 0; i < fields->size; ++i) {
    http_bytes_clear(&fields->items[i].name);
    http_bytes_clear(&fields->items[i].value);
  }
  free(fields->items);
  *fields = (http_fields){0};
}

http_event *http_event_new(int32_t kind) {
  http_event *event = calloc(1, sizeof(*event));
  if (event)
    event->info.kind = kind;
  return event;
}

void http_event_free(http_event *event) {
  if (!event)
    return;
  for (size_t i = 0; i < 4; ++i)
    http_bytes_clear(&event->bytes[i]);
  http_fields_clear(&event->fields);
  free(event);
}

void http_events_push(http_events *events, http_event *event) {
  if (events->last)
    events->last->next = event;
  else
    events->first = event;
  events->last = event;
}

http_event *http_events_pop(http_events *events) {
  http_event *event = events->first;
  if (event) {
    events->first = event->next;
    event->next = NULL;
    if (!events->first)
      events->last = NULL;
  }
  return event;
}

void http_events_clear(http_events *events) {
  http_event *event;
  while ((event = http_events_pop(events)))
    http_event_free(event);
}

int http_fail(http_error *error, int code, const char *message) {
  error->code = code;
  snprintf(error->text, sizeof(error->text), "%s", message);
  return code;
}

http_event_info scoop_http_event_info(uint64_t handle) {
  return ((http_event *)(uintptr_t)handle)->info;
}

http_byte_view scoop_http_event_bytes(uint64_t handle, int32_t index) {
  http_bytes *bytes = &((http_event *)(uintptr_t)handle)->bytes[index];
  return (http_byte_view){bytes->data, (int64_t)bytes->size};
}

int64_t scoop_http_event_field_count(uint64_t handle) {
  return (int64_t)((http_event *)(uintptr_t)handle)->fields.size;
}

http_field_view scoop_http_event_field(uint64_t handle, int64_t index) {
  http_field *field = &((http_event *)(uintptr_t)handle)->fields.items[index];
  return (http_field_view){field->name.data, (int64_t)field->name.size,
                           field->value.data, (int64_t)field->value.size,
                           field->never_index};
}

void scoop_http_event_free(uint64_t handle) {
  http_event_free((http_event *)(uintptr_t)handle);
}
