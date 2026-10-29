#ifndef SCOOP_TEST_WAYLAND_HOST_H
#define SCOOP_TEST_WAYLAND_HOST_H

#include "../../protocols/fractional-scale-v1.h"
#include "../../protocols/text-input-unstable-v3.h"
#include "../../protocols/viewporter.h"
#include "../../protocols/xdg-decoration-unstable-v1.h"
#include "../../protocols/xdg-shell.h"
#include <pthread.h>
#include <stdatomic.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <sys/eventfd.h>
#include <unistd.h>
#include <wayland-server.h>

// A protocol peer for the real client backend. Pixel presentation is tested on
// Weston.
typedef struct WTServer WTServer;
typedef struct {
  WTServer *server;
  struct wl_resource *surface, *xdg, *top, *decoration, *viewport, *fractional;
  struct wl_resource *buffer, *frame;
  int destination_width, destination_height, buffer_scale;
  bool configured, maximized;
} WTSurface;
struct WTServer {
  struct wl_display *display;
  struct wl_event_source *commands;
  struct wl_client *client;
  struct wl_global *output_global;
  struct wl_resource *output, *seat, *pointer, *keyboard, *text, *data_device;
  WTSurface *window;
  pthread_t thread;
  int wake, features;
  char *name, *keymap, *surrounding;
  uint32_t keymap_size, text_serial;
  int anchor, cursor;
  bool enabled;
  atomic_int command, minimum_serial, failures, last_text_serial;
  atomic_int finished_drops, destination_width, destination_height;
  atomic_int buffer_scale, surfaces;
};

void wt_destroy(struct wl_client *client, struct wl_resource *resource);
void wt_bind_seat(struct wl_client *client, void *data, uint32_t version,
                  uint32_t id);
void wt_bind_text(struct wl_client *client, void *data, uint32_t version,
                  uint32_t id);
void wt_bind_data(struct wl_client *client, void *data, uint32_t version,
                  uint32_t id);
void wt_execute(WTServer *server);
void wt_configure(WTSurface *surface, int width, int height);
void wt_enter(WTServer *server);
void wt_output_change(WTServer *server, int command);
#endif
