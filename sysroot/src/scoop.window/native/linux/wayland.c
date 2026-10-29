#include "wayland.h"

static void ping(void *data, struct xdg_wm_base *wm, uint32_t serial) {
  xdg_wm_base_pong(wm, serial);
}
static const struct xdg_wm_base_listener wm_listener = {.ping = ping};

static void output_geometry(void *data, struct wl_output *native, int32_t x,
                            int32_t y, int32_t width, int32_t height,
                            int32_t subpixel, const char *make,
                            const char *model, int32_t transform) {
  SWOutput *output = data;
  if (!output->monitor->data.name && model && *model) {
    output->monitor->data.name = (const uint8_t *)strdup(model);
    output->monitor->data.name_length = (int64_t)strlen(model);
  }
}
static void output_mode(void *data, struct wl_output *native, uint32_t flags,
                        int32_t width, int32_t height, int32_t refresh) {
  if (!(flags & WL_OUTPUT_MODE_CURRENT))
    return;
  SWOutput *output = data;
  output->monitor->data.pixel_width = width;
  output->monitor->data.pixel_height = height;
  output->monitor->data.refresh_millihertz = refresh > 0 ? refresh : 0;
}
static void output_done(void *data, struct wl_output *native) {
  SWOutput *output = data;
  SWMonitorData *monitor = &output->monitor->data;
  monitor->scale = output->scale;
  monitor->position_known = output->logical_position && output->logical_size;
  monitor->x = output->logical_x;
  monitor->y = output->logical_y;
  monitor->width = output->logical_size ? output->logical_width
                                        : monitor->pixel_width / output->scale;
  monitor->height = output->logical_size
                        ? output->logical_height
                        : monitor->pixel_height / output->scale;
  sw_monitor_changed(output->wayland->app);
  for (SWWindow *window = output->wayland->app->windows; window;
       window = window->next)
    if (window->wl)
      sw_wayland_scale(window);
}
static void output_scale(void *data, struct wl_output *native, int32_t scale) {
  if (scale > 0)
    ((SWOutput *)data)->scale = scale;
}
static void output_name(void *data, struct wl_output *native,
                        const char *name) {
  SWOutput *output = data;
  char *copy = strdup(name);
  if (!copy)
    return;
  free((void *)output->monitor->data.name);
  output->monitor->data.name = (const uint8_t *)copy;
  output->monitor->data.name_length = (int64_t)strlen(copy);
}
static void output_description(void *data, struct wl_output *native,
                               const char *description) {}
static const struct wl_output_listener output_listener = {
    .geometry = output_geometry,
    .mode = output_mode,
    .done = output_done,
    .scale = output_scale,
    .name = output_name,
    .description = output_description};
static void logical_position(void *data, struct zxdg_output_v1 *native,
                             int32_t x, int32_t y) {
  SWOutput *output = data;
  output->logical_x = x;
  output->logical_y = y;
  output->logical_position = true;
}
static void logical_size(void *data, struct zxdg_output_v1 *native,
                         int32_t width, int32_t height) {
  SWOutput *output = data;
  output->logical_width = width;
  output->logical_height = height;
  output->logical_size = width > 0 && height > 0;
}
static void logical_done(void *data, struct zxdg_output_v1 *native) {
  output_done(data, NULL);
}
static void logical_name(void *data, struct zxdg_output_v1 *native,
                         const char *name) {
  output_name(data, NULL, name);
}
static void logical_description(void *data, struct zxdg_output_v1 *native,
                                const char *description) {}
static const struct zxdg_output_v1_listener logical_listener = {
    .logical_position = logical_position,
    .logical_size = logical_size,
    .done = logical_done,
    .name = logical_name,
    .description = logical_description};

void sw_wayland_output_description(SWOutput *output) {
  if (!output->description && output->wayland->output_manager) {
    output->description = zxdg_output_manager_v1_get_xdg_output(
        output->wayland->output_manager, output->output);
    zxdg_output_v1_add_listener(output->description, &logical_listener, output);
  }
}

static void add_output(SWWayland *wl, uint32_t name, uint32_t version) {
  if (version < 2)
    return;
  SWOutput *output = calloc(1, sizeof(*output));
  SWMonitor *monitor = calloc(1, sizeof(*monitor));
  if (!output || !monitor) {
    free(output);
    free(monitor);
    sw_error(wl->app, SWErrorNative, ENOMEM);
    wl->app->error_pending = true;
    return;
  }
  output->wayland = wl;
  output->global = name;
  output->scale = 1;
  output->monitor = monitor;
  monitor->native_id = name;
  monitor->data.key = wl->app->next_monitor++;
  monitor->data.scale = 1;
  monitor->next = wl->app->monitors;
  wl->app->monitors = monitor;
  output->output = wl_registry_bind(wl->registry, name, &wl_output_interface,
                                    version < 4 ? version : 4);
  wl_output_add_listener(output->output, &output_listener, output);
  output->next = wl->outputs;
  wl->outputs = output;
  sw_wayland_output_description(output);
}

static void registry_global(void *data, struct wl_registry *registry,
                            uint32_t name, const char *interface,
                            uint32_t version) {
  SWWayland *wl = data;
  if (!strcmp(interface, "wl_compositor") && version >= 4)
    wl->compositor =
        wl_registry_bind(registry, name, &wl_compositor_interface, 4);
  else if (!strcmp(interface, "wl_shm"))
    wl->shm = wl_registry_bind(registry, name, &wl_shm_interface, 1);
  else if (!strcmp(interface, "xdg_wm_base")) {
    wl->wm = wl_registry_bind(registry, name, &xdg_wm_base_interface,
                              version < 6 ? version : 6);
    xdg_wm_base_add_listener(wl->wm, &wm_listener, wl);
  } else if (!strcmp(interface, "wp_viewporter"))
    wl->viewporter =
        wl_registry_bind(registry, name, &wp_viewporter_interface, 1);
  else if (!strcmp(interface, "wp_fractional_scale_manager_v1"))
    wl->fractional = wl_registry_bind(
        registry, name, &wp_fractional_scale_manager_v1_interface, 1);
  else if (!strcmp(interface, "zxdg_decoration_manager_v1"))
    wl->decorations = wl_registry_bind(
        registry, name, &zxdg_decoration_manager_v1_interface, 1);
  else if (!strcmp(interface, "zxdg_output_manager_v1")) {
    wl->output_manager =
        wl_registry_bind(registry, name, &zxdg_output_manager_v1_interface,
                         version < 3 ? version : 3);
    for (SWOutput *output = wl->outputs; output; output = output->next)
      sw_wayland_output_description(output);
  } else if (!strcmp(interface, "zwp_text_input_manager_v3")) {
    wl->text_manager = wl_registry_bind(
        registry, name, &zwp_text_input_manager_v3_interface, 1);
    for (SWSeat *seat = wl->seats; seat; seat = seat->next)
      sw_wayland_text_setup(seat);
  } else if (!strcmp(interface, "wl_data_device_manager")) {
    wl->data_manager =
        wl_registry_bind(registry, name, &wl_data_device_manager_interface,
                         version < 3 ? version : 3);
    for (SWSeat *seat = wl->seats; seat; seat = seat->next)
      sw_wayland_drop_setup(seat);
  } else if (!strcmp(interface, "xdg_activation_v1"))
    wl->activation =
        wl_registry_bind(registry, name, &xdg_activation_v1_interface, 1);
  else if (!strcmp(interface, "wl_output"))
    add_output(wl, name, version);
  else if (!strcmp(interface, "wl_seat"))
    sw_wayland_seat(wl, name, version);
}

static void destroy_output(SWOutput *output) {
  if (output->description)
    zxdg_output_v1_destroy(output->description);
  if (wl_output_get_version(output->output) >= 3)
    wl_output_release(output->output);
  else
    wl_output_destroy(output->output);
  free(output);
}

static void registry_remove(void *data, struct wl_registry *registry,
                            uint32_t name) {
  SWWayland *wl = data;
  SWSeat **seat = &wl->seats;
  while (*seat) {
    if ((*seat)->global == name) {
      SWSeat *old = *seat;
      *seat = old->next;
      sw_wayland_seat_destroy(old);
      break;
    }
    seat = &(*seat)->next;
  }
  SWOutput **link = &wl->outputs;
  while (*link) {
    SWOutput *output = *link;
    if (output->global != name) {
      link = &output->next;
      continue;
    }
    *link = output->next;
    for (SWWindow *window = wl->app->windows; window; window = window->next) {
      if (!window->wl)
        continue;
      SWOutputRef **ref = &window->wl->outputs;
      while (*ref) {
        if ((*ref)->output == output) {
          SWOutputRef *old = *ref;
          *ref = old->next;
          free(old);
        } else
          ref = &(*ref)->next;
      }
      sw_wayland_scale(window);
    }
    SWMonitor **monitor = &wl->app->monitors;
    while (*monitor != output->monitor)
      monitor = &(*monitor)->next;
    *monitor = output->monitor->next;
    output->monitor->next = NULL;
    sw_monitors_free(output->monitor);
    destroy_output(output);
    sw_monitor_changed(wl->app);
    break;
  }
}
static const struct wl_registry_listener registry_listener = {
    .global = registry_global, .global_remove = registry_remove};

bool sw_wayland_connect(SWApp *app, const char *display) {
  SWWayland *wl = calloc(1, sizeof(*wl));
  if (!wl) {
    sw_error(app, SWErrorNative, ENOMEM);
    return false;
  }
  wl->app = app;
  wl->display = wl_display_connect(display);
  if (!wl->display) {
    sw_error(app, SWErrorUnavailable, errno);
    free(wl);
    return false;
  }
  app->wayland = wl;
  app->display_fd = wl_display_get_fd(wl->display);
  wl->registry = wl_display_get_registry(wl->display);
  wl_registry_add_listener(wl->registry, &registry_listener, wl);
  bool success = wl_display_roundtrip(wl->display) >= 0 &&
                 wl_display_roundtrip(wl->display) >= 0;
  if (!success || !wl->compositor || !wl->wm || !wl->shm ||
      app->error_pending) {
    if (!app->failure.kind)
      sw_error(app, SWErrorUnavailable, success ? ENOTSUP : errno);
    sw_wayland_destroy(app);
    sw_monitors_free(app->monitors);
    app->monitors = NULL;
    return false;
  }
  return true;
}

void sw_wayland_dispatch(SWApp *app) {
  SWWayland *wl = app->wayland;
  if (wl_display_dispatch_pending(wl->display) < 0) {
    sw_disconnect(app, errno);
    return;
  }
  if (wl->decor && libdecor_dispatch(wl->decor, 0) < 0)
    sw_disconnect(app, errno);
}

int sw_wayland_prepare(SWApp *app, bool *write) {
  struct wl_display *display = app->wayland->display;
  if (wl_display_prepare_read(display) != 0) {
    if (wl_display_get_error(display))
      sw_disconnect(app, wl_display_get_error(display));
    return 0;
  }
  if (wl_display_flush(display) < 0) {
    if (errno == EAGAIN)
      *write = true;
    else {
      int error = errno;
      wl_display_cancel_read(display);
      sw_disconnect(app, error);
      return -1;
    }
  }
  return 1;
}
void sw_wayland_read(SWApp *app, bool readable) {
  struct wl_display *display = app->wayland->display;
  if (readable) {
    if (wl_display_read_events(display) < 0)
      sw_disconnect(app, errno);
  } else
    wl_display_cancel_read(display);
}
int sw_wayland_extra_fd(SWApp *app) {
  return app->wayland->decor ? libdecor_get_fd(app->wayland->decor) : -1;
}
void *sw_wayland_display(SWApp *app) { return app->wayland->display; }
void *sw_wayland_surface(SWWindow *window) { return window->wl->surface; }

void sw_wayland_destroy(SWApp *app) {
  SWWayland *wl = app->wayland;
  while (wl->seats) {
    SWSeat *seat = wl->seats;
    wl->seats = seat->next;
    sw_wayland_seat_destroy(seat);
  }
  while (wl->outputs) {
    SWOutput *output = wl->outputs;
    wl->outputs = output->next;
    destroy_output(output);
  }
  if (wl->decor)
    libdecor_unref(wl->decor);
  if (wl->activation)
    xdg_activation_v1_destroy(wl->activation);
  if (wl->data_manager)
    wl_data_device_manager_destroy(wl->data_manager);
  if (wl->text_manager)
    zwp_text_input_manager_v3_destroy(wl->text_manager);
  if (wl->decorations)
    zxdg_decoration_manager_v1_destroy(wl->decorations);
  if (wl->output_manager)
    zxdg_output_manager_v1_destroy(wl->output_manager);
  if (wl->fractional)
    wp_fractional_scale_manager_v1_destroy(wl->fractional);
  if (wl->viewporter)
    wp_viewporter_destroy(wl->viewporter);
  if (wl->wm)
    xdg_wm_base_destroy(wl->wm);
  if (wl->shm)
    wl_shm_destroy(wl->shm);
  if (wl->compositor)
    wl_compositor_destroy(wl->compositor);
  wl_registry_destroy(wl->registry);
  wl_display_flush(wl->display);
  wl_display_disconnect(wl->display);
  free(wl);
  app->wayland = NULL;
}
