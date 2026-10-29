#include "window.h"

int64_t sw_now(void) {
  struct timespec value;
  clock_gettime(CLOCK_MONOTONIC, &value);
  return (int64_t)value.tv_sec * 1000000000 + value.tv_nsec;
}

int sw_error(SWApp *app, int kind, int code) {
  app->failure = (SWFailure){app->backend, kind, code};
  return SWFailed;
}

void sw_disconnect(SWApp *app, int code) {
  if (app->disconnected)
    return;
  sw_error(app, SWErrorDisconnected, code);
  app->disconnected = true;
  app->error_pending = true;
  for (SWWindow *window = app->windows; window; window = window->next)
    window->snapshot.redraw_ready = 0;
  sw_wake(app);
}

char *sw_string(const uint8_t *text, int64_t length) {
  if (length < 0 || (uint64_t)length >= SIZE_MAX ||
      (length && (!text || memchr(text, 0, (size_t)length)))) {
    errno = EINVAL;
    return NULL;
  }
  char *result = malloc((size_t)length + 1);
  if (!result)
    return NULL;
  if (length)
    memcpy(result, text, (size_t)length);
  result[length] = 0;
  return result;
}

SWEvent *sw_event(SWWindow *window, int kind) {
  SWEvent *event = calloc(1, sizeof(*event));
  if (!event) {
    sw_error(window->app, SWErrorNative, ENOMEM);
    window->app->error_pending = true;
    sw_wake(window->app);
    return NULL;
  }
  event->data.window = window->identity;
  event->data.kind = kind;
  event->data.d = window->snapshot.generation;
  event->revision = -1;
  return event;
}

void sw_enqueue(SWApp *app, SWEvent *event) {
  if (!event)
    return;
  *app->events_tail = event;
  app->events_tail = &event->next;
  app->pump = true;
}

void sw_event_free(SWEvent *event) {
  for (size_t i = 0; i < event->edit_count; ++i)
    free((void *)event->edits[i].text);
  free(event->edits);
  free((void *)event->data.text);
  free(event);
}

void sw_emit(SWWindow *window, int kind, int flags) {
  SWEvent *event = sw_event(window, kind);
  if (!event)
    return;
  event->data.flags = flags;
  if (kind == SWMoved) {
    event->data.x = window->snapshot.x;
    event->data.y = window->snapshot.y;
  }
  sw_enqueue(window->app, event);
}

void sw_geometry(SWWindow *window) {
  SWSnapshot *value = &window->snapshot;
  if (!value->configured)
    return;
  double width = ceil(value->width * value->scale);
  double height = ceil(value->height * value->scale);
  if (!isfinite(width) || !isfinite(height) || width > INT_MAX ||
      height > INT_MAX) {
    sw_error(window->app, SWErrorNative, EOVERFLOW);
    window->app->error_pending = true;
    return;
  }
  value->pixel_width = width;
  value->pixel_height = height;
  SWEvent *event = sw_event(window, SWGeometry);
  if (!event)
    return;
  event->data.x = value->width;
  event->data.y = value->height;
  event->data.z = width;
  event->data.w = height;
  event->data.scale = value->scale;
  sw_enqueue(window->app, event);
}

void sw_text_edit(SWEvent *event, int kind, const char *text, int64_t anchor,
                  int64_t cursor) {
  if (!event)
    return;
  char *copy = text ? strdup(text) : NULL;
  SWTextEditData *edits =
      realloc(event->edits, (event->edit_count + 1) * sizeof(*edits));
  if (!edits || (text && !copy)) {
    free(copy);
    if (edits)
      event->edits = edits;
    event->data.flags = -1;
    return;
  }
  event->edits = edits;
  edits[event->edit_count++] =
      (SWTextEditData){kind, anchor, cursor, (const uint8_t *)copy,
                       copy ? (int64_t)strlen(copy) : 0};
}

void sw_commit(SWWindow *window, const char *text) {
  if (!text || !*text)
    return;
  SWEvent *event = sw_event(window, SWTextBatch);
  sw_text_edit(event, 1, text, -1, -1);
  sw_enqueue(window->app, event);
}

void sw_cancel_preedit(SWWindow *window) {
  if (!window->preedit_active)
    return;
  window->preedit_active = false;
  SWEvent *event = sw_event(window, SWTextBatch);
  sw_text_edit(event, 0, "", -1, -1);
  sw_enqueue(window->app, event);
}

void sw_pointer(SWWindow *window, int kind, double x, double y,
                uint64_t modifiers, int64_t button, double dx, double dy,
                int phase) {
  SWEvent *event = sw_event(window, SWPointer);
  if (!event)
    return;
  event->data.flags = kind;
  event->data.a = button;
  event->data.b = (int64_t)modifiers;
  event->data.c = kind == 3 || kind == 4 ? 1 : 0;
  if (kind == 5 || kind == 6)
    event->data.a = phase;
  event->data.x = x;
  event->data.y = y;
  event->data.scale = window->snapshot.scale;
  event->data.z = x * event->data.scale;
  event->data.w = y * event->data.scale;
  event->data.dx = dx;
  event->data.dy = dy;
  sw_enqueue(window->app, event);
}

void sw_monitor_changed(SWApp *app) {
  SWEvent *event = calloc(1, sizeof(*event));
  if (!event) {
    sw_error(app, SWErrorNative, ENOMEM);
    app->error_pending = true;
    return;
  }
  event->data.kind = SWMonitors;
  sw_enqueue(app, event);
}

void *scoop_window_next_event(void *pointer) {
  SWApp *app = pointer;
  SWEvent *event = app->events;
  if (!event)
    return NULL;
  app->events = event->next;
  if (!app->events)
    app->events_tail = &app->events;
  event->next = NULL;
  return event;
}
SWEventData scoop_window_event_data(void *pointer) {
  return ((SWEvent *)pointer)->data;
}
void scoop_window_event_release(void *pointer) { sw_event_free(pointer); }
int64_t scoop_window_text_count(void *pointer) {
  SWEvent *event = pointer;
  return event->data.flags == -1 ? -1 : (int64_t)event->edit_count;
}
SWTextEditData scoop_window_text_edit(void *pointer, int64_t index) {
  return ((SWEvent *)pointer)->edits[index];
}
int64_t scoop_window_text_revision(void *pointer) {
  return ((SWEvent *)pointer)->revision;
}
