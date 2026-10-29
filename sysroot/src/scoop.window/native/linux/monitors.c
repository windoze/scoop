#include "window.h"

void sw_monitors_free(SWMonitor *monitors) {
  while (monitors) {
    SWMonitor *next = monitors->next;
    free((void *)monitors->data.name);
    free(monitors);
    monitors = next;
  }
}

typedef struct {
  size_t count;
  SWMonitorData *items;
} SWMonitorSnapshot;

void *scoop_window_monitors(void *pointer) {
  SWApp *app = pointer;
  SWMonitorSnapshot *snapshot = calloc(1, sizeof(*snapshot));
  if (!snapshot) {
    sw_error(app, SWErrorNative, ENOMEM);
    return NULL;
  }
  for (SWMonitor *monitor = app->monitors; monitor; monitor = monitor->next)
    ++snapshot->count;
  snapshot->items =
      calloc(snapshot->count ? snapshot->count : 1, sizeof(*snapshot->items));
  if (!snapshot->items) {
    free(snapshot);
    sw_error(app, SWErrorNative, ENOMEM);
    return NULL;
  }
  size_t index = 0;
  for (SWMonitor *monitor = app->monitors; monitor; monitor = monitor->next) {
    snapshot->items[index] = monitor->data;
    if (monitor->data.name) {
      snapshot->items[index].name =
          (const uint8_t *)strdup((const char *)monitor->data.name);
      if (!snapshot->items[index].name) {
        for (size_t i = 0; i < index; ++i)
          free((void *)snapshot->items[i].name);
        free(snapshot->items);
        free(snapshot);
        sw_error(app, SWErrorNative, ENOMEM);
        return NULL;
      }
    }
    ++index;
  }
  return snapshot;
}
int64_t scoop_window_monitors_count(void *pointer) {
  return (int64_t)((SWMonitorSnapshot *)pointer)->count;
}
SWMonitorData scoop_window_monitor_data(void *pointer, int64_t index) {
  return ((SWMonitorSnapshot *)pointer)->items[index];
}
void scoop_window_monitors_release(void *pointer) {
  SWMonitorSnapshot *snapshot = pointer;
  for (size_t i = 0; i < snapshot->count; ++i)
    free((void *)snapshot->items[i].name);
  free(snapshot->items);
  free(snapshot);
}
