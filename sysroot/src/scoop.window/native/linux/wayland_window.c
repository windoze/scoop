#include "wayland.h"

static SWWayland *decor_owner;
static bool map_window(SWWindow *window);

SWWindow *sw_wayland_find(SWWayland *wl, struct wl_surface *surface) {
  for (SWWindow *window = wl->app->windows; window; window = window->next)
    if (window->wl && window->wl->surface == surface)
      return window;
  return NULL;
}

static void buffer_geometry(SWWindow *window) {
  SWWLWindow *native = window->wl;
  if (native->viewport && native->fractional_scale > 0) {
    wl_surface_set_buffer_scale(native->surface, 1);
    if (window->snapshot.configured)
      wp_viewport_set_destination(native->viewport, (int)window->snapshot.width,
                                  (int)window->snapshot.height);
  } else
    wl_surface_set_buffer_scale(native->surface, (int)window->snapshot.scale);
}

void sw_wayland_scale(SWWindow *window) {
  double scale = window->wl->fractional_scale;
  if (scale <= 0) {
    scale = 1;
    for (SWOutputRef *ref = window->wl->outputs; ref; ref = ref->next)
      if (ref->output->scale > scale)
        scale = ref->output->scale;
  }
  if (scale == window->snapshot.scale)
    return;
  window->snapshot.scale = scale;
  buffer_geometry(window);
  sw_geometry(window);
  for (SWSeat *seat = window->app->wayland->seats; seat; seat = seat->next)
    if (seat->pointer_window == window)
      sw_wayland_cursor(seat);
}

static void surface_enter(void *data, struct wl_surface *surface,
                          struct wl_output *output) {
  SWWindow *window = data;
  SWOutput *record = wl_output_get_user_data(output);
  if (!record)
    return;
  for (SWOutputRef *ref = window->wl->outputs; ref; ref = ref->next)
    if (ref->output == record)
      return;
  SWOutputRef *ref = malloc(sizeof(*ref));
  if (!ref) {
    sw_error(window->app, SWErrorNative, ENOMEM);
    window->app->error_pending = true;
    return;
  }
  *ref = (SWOutputRef){record, window->wl->outputs};
  window->wl->outputs = ref;
  sw_wayland_scale(window);
}
static void surface_leave(void *data, struct wl_surface *surface,
                          struct wl_output *output) {
  SWWindow *window = data;
  SWOutputRef **link = &window->wl->outputs;
  while (*link) {
    if ((*link)->output->output == output) {
      SWOutputRef *old = *link;
      *link = old->next;
      free(old);
      break;
    }
    link = &(*link)->next;
  }
  sw_wayland_scale(window);
}
static const struct wl_surface_listener surface_listener = {
    .enter = surface_enter, .leave = surface_leave};

static void preferred_scale(void *data, struct wp_fractional_scale_v1 *native,
                            uint32_t scale) {
  SWWindow *window = data;
  if (!scale) {
    sw_error(window->app, SWErrorNative, EINVAL);
    window->app->error_pending = true;
    return;
  }
  window->wl->fractional_scale = scale / 120.0;
  sw_wayland_scale(window);
}
static const struct wp_fractional_scale_v1_listener scale_listener = {
    .preferred_scale = preferred_scale};

int sw_wayland_dimension(int configured, double requested, double previous,
                         double fallback, double minimum, double maximum) {
  if (configured > 0)
    return configured;
  double value = requested > 0 ? requested : previous > 0 ? previous : fallback;
  value = fmax(minimum, value);
  if (maximum > 0)
    value = fmin(maximum, value);
  return (int)fmax(1, fmin(INT_MAX, ceil(value)));
}

void sw_wayland_configured(SWWindow *window, int width, int height,
                           bool maximized) {
  SWSnapshot *value = &window->snapshot;
  value->width = width;
  value->height = height;
  value->configured = 1;
  value->maximized = maximized;
  value->redraw_ready = value->visible && !window->wl->frame_callback;
  buffer_geometry(window);
  sw_geometry(window);
}

static void destroy_role(SWWindow *window) {
  SWWLWindow *native = window->wl;
  if (native->frame) {
    libdecor_frame_unref(native->frame);
    native->frame = NULL;
  }
  if (native->decoration) {
    zxdg_toplevel_decoration_v1_destroy(native->decoration);
    native->decoration = NULL;
  }
  if (native->toplevel) {
    xdg_toplevel_destroy(native->toplevel);
    native->toplevel = NULL;
  }
  if (native->xdg_surface) {
    xdg_surface_destroy(native->xdg_surface);
    native->xdg_surface = NULL;
  }
  native->pending_width = native->pending_height = 0;
  native->pending_states = 0;
  native->capabilities_known = false;
}

static void decor_error(struct libdecor *context, enum libdecor_error error,
                        const char *message) {
  if (!decor_owner)
    return;
  sw_error(decor_owner->app, SWErrorNative, error);
  decor_owner->app->error_pending = true;
  sw_wake(decor_owner->app);
}
static struct libdecor_interface decor_interface = {.error = decor_error};

static void decor_configure(struct libdecor_frame *frame,
                            struct libdecor_configuration *configuration,
                            void *data) {
  SWWindow *window = data;
  if (!window->snapshot.visible)
    return;
  int width = 0, height = 0;
  libdecor_configuration_get_content_size(configuration, frame, &width,
                                          &height);
  width = sw_wayland_dimension(width, window->requested_width,
                               window->snapshot.width, 800, window->min_width,
                               window->max_width);
  height = sw_wayland_dimension(height, window->requested_height,
                                window->snapshot.height, 600,
                                window->min_height, window->max_height);
  enum libdecor_window_state state = LIBDECOR_WINDOW_STATE_NONE;
  libdecor_configuration_get_window_state(configuration, &state);
  struct libdecor_state *content = libdecor_state_new(width, height);
  if (!content) {
    sw_error(window->app, SWErrorNative, ENOMEM);
    window->app->error_pending = true;
    return;
  }
  libdecor_frame_commit(frame, content, configuration);
  libdecor_state_free(content);
  if (window->decorations) {
    int x = 0, y = 0;
    libdecor_frame_translate_coordinate(frame, 0, 0, &x, &y);
    // This path is selected only when the compositor requires client
    // decorations. libdecor's no-plugin fallback must not silently create a
    // borderless window.
    if (x == 0 && y == 0) {
      sw_error(window->app, SWErrorUnsupported, ENOENT);
      window->app->error_pending = true;
      return;
    }
  }
  sw_wayland_configured(window, width, height,
                        (state & LIBDECOR_WINDOW_STATE_MAXIMIZED) != 0);
}
static void decor_close(struct libdecor_frame *frame, void *data) {
  sw_emit(data, SWClose, 0);
}
static void decor_commit(struct libdecor_frame *frame, void *data) {
  SWWindow *window = data;
  if (window->snapshot.configured && window->snapshot.visible)
    wl_surface_commit(window->wl->surface);
}
static void decor_dismiss(struct libdecor_frame *frame, const char *seat,
                          void *data) {
  // This library creates no xdg_popup objects for libdecor to dismiss.
}
static struct libdecor_frame_interface frame_interface = {
    .configure = decor_configure,
    .close = decor_close,
    .commit = decor_commit,
    .dismiss_popup = decor_dismiss};

static void surface_configure(void *data, struct xdg_surface *surface,
                              uint32_t serial) {
  SWWindow *window = data;
  SWWLWindow *native = window->wl;
  xdg_surface_ack_configure(surface, serial);
  if (!window->snapshot.visible)
    return;
  if (native->need_decor) {
    destroy_role(window);
    native->use_decor = true;
    native->need_decor = false;
    if (!map_window(window))
      window->app->error_pending = true;
    return;
  }
  int width = sw_wayland_dimension(
      native->pending_width, window->requested_width, window->snapshot.width,
      800, window->min_width, window->max_width);
  int height = sw_wayland_dimension(
      native->pending_height, window->requested_height, window->snapshot.height,
      600, window->min_height, window->max_height);
  xdg_surface_set_window_geometry(surface, 0, 0, width, height);
  sw_wayland_configured(window, width, height,
                        (native->pending_states & 1) != 0);
}
static const struct xdg_surface_listener xdg_surface_listener = {
    .configure = surface_configure};
static void toplevel_configure(void *data, struct xdg_toplevel *toplevel,
                               int32_t width, int32_t height,
                               struct wl_array *states) {
  SWWLWindow *native = ((SWWindow *)data)->wl;
  native->pending_width = width;
  native->pending_height = height;
  native->pending_states = 0;
  uint32_t *state;
  wl_array_for_each(state, states) {
    if (*state == XDG_TOPLEVEL_STATE_MAXIMIZED)
      native->pending_states |= 1;
  }
}
static void toplevel_close(void *data, struct xdg_toplevel *toplevel) {
  sw_emit(data, SWClose, 0);
}
static void configure_bounds(void *data, struct xdg_toplevel *toplevel,
                             int32_t width, int32_t height) {
  // Bounds are advisory; the application already supplies a valid initial size.
}
static void wm_capabilities(void *data, struct xdg_toplevel *toplevel,
                            struct wl_array *capabilities) {
  SWWindow *window = data;
  SWWLWindow *native = window->wl;
  bool first = !native->capabilities_known;
  native->capabilities_known = true;
  native->can_maximize = native->can_minimize = false;
  uint32_t *capability;
  wl_array_for_each(capability, capabilities) {
    if (*capability == XDG_TOPLEVEL_WM_CAPABILITIES_MAXIMIZE)
      native->can_maximize = true;
    if (*capability == XDG_TOPLEVEL_WM_CAPABILITIES_MINIMIZE)
      native->can_minimize = true;
  }
  if (first && native->requested_maximized && !native->can_maximize) {
    sw_error(window->app, SWErrorUnsupported, ENOTSUP);
    window->app->error_pending = true;
  }
}
static const struct xdg_toplevel_listener toplevel_listener = {
    .configure = toplevel_configure,
    .close = toplevel_close,
    .configure_bounds = configure_bounds,
    .wm_capabilities = wm_capabilities};
static void decoration_configure(void *data,
                                 struct zxdg_toplevel_decoration_v1 *decoration,
                                 uint32_t mode) {
  SWWindow *window = data;
  if (window->decorations &&
      mode == ZXDG_TOPLEVEL_DECORATION_V1_MODE_CLIENT_SIDE)
    window->wl->need_decor = true;
  else if (!window->decorations &&
           mode == ZXDG_TOPLEVEL_DECORATION_V1_MODE_SERVER_SIDE) {
    sw_error(window->app, SWErrorUnsupported, ENOTSUP);
    window->app->error_pending = true;
  }
}
static const struct zxdg_toplevel_decoration_v1_listener decoration_listener = {
    .configure = decoration_configure};

static bool map_window(SWWindow *window) {
  SWWayland *wl = window->app->wayland;
  SWWLWindow *native = window->wl;
  if (native->use_decor) {
    if (!wl->decor) {
      decor_owner = wl;
      wl->decor = libdecor_new(wl->display, &decor_interface);
      if (!wl->decor) {
        sw_error(window->app, SWErrorUnavailable, ENOENT);
        return false;
      }
    }
    native->frame =
        libdecor_decorate(wl->decor, native->surface, &frame_interface, window);
    if (!native->frame) {
      sw_error(window->app, SWErrorNative, ENOMEM);
      return false;
    }
    libdecor_frame_set_title(native->frame, window->title);
    libdecor_frame_set_app_id(native->frame, window->app_id);
    libdecor_frame_set_visibility(native->frame, window->decorations);
    sw_wayland_limits(window);
    sw_wayland_buttons(window);
    if (native->requested_maximized)
      libdecor_frame_set_maximized(native->frame);
    libdecor_frame_map(native->frame);
  } else {
    native->xdg_surface = xdg_wm_base_get_xdg_surface(wl->wm, native->surface);
    xdg_surface_add_listener(native->xdg_surface, &xdg_surface_listener,
                             window);
    native->toplevel = xdg_surface_get_toplevel(native->xdg_surface);
    xdg_toplevel_add_listener(native->toplevel, &toplevel_listener, window);
    xdg_toplevel_set_title(native->toplevel, window->title);
    xdg_toplevel_set_app_id(native->toplevel, window->app_id);
    if (wl->decorations) {
      native->decoration = zxdg_decoration_manager_v1_get_toplevel_decoration(
          wl->decorations, native->toplevel);
      zxdg_toplevel_decoration_v1_add_listener(native->decoration,
                                               &decoration_listener, window);
      zxdg_toplevel_decoration_v1_set_mode(
          native->decoration,
          window->decorations ? ZXDG_TOPLEVEL_DECORATION_V1_MODE_SERVER_SIDE
                              : ZXDG_TOPLEVEL_DECORATION_V1_MODE_CLIENT_SIDE);
    }
    sw_wayland_limits(window);
    if (native->requested_maximized)
      xdg_toplevel_set_maximized(native->toplevel);
    wl_surface_commit(native->surface);
  }
  return true;
}

int sw_wayland_visibility(SWWindow *window, bool visible) {
  SWWLWindow *native = window->wl;
  if (visible == (window->snapshot.visible != 0))
    return SWApplied;
  if (visible) {
    window->snapshot.visible = 1;
    if (!map_window(window)) {
      window->snapshot.visible = 0;
      return SWFailed;
    }
  } else {
    sw_wayland_input_destroy(window);
    sw_wayland_drop_window_destroy(window);
    window->snapshot.visible = window->snapshot.configured =
        window->snapshot.redraw_ready = 0;
    ++window->snapshot.generation;
    if (native->frame_callback) {
      wl_callback_destroy(native->frame_callback);
      native->frame_callback = NULL;
    }
    wl_surface_attach(native->surface, NULL, 0, 0);
    wl_surface_commit(native->surface);
    destroy_role(window);
  }
  return SWRequested;
}

bool sw_wayland_create(SWWindow *window) {
  SWWayland *wl = window->app->wayland;
  SWWLWindow *native = calloc(1, sizeof(*native));
  if (!native) {
    sw_error(window->app, SWErrorNative, ENOMEM);
    return false;
  }
  window->wl = native;
  native->surface = wl_compositor_create_surface(wl->compositor);
  native->use_decor = !wl->decorations;
  wl_surface_add_listener(native->surface, &surface_listener, window);
  if (wl->fractional && wl->viewporter) {
    native->viewport =
        wp_viewporter_get_viewport(wl->viewporter, native->surface);
    native->fractional = wp_fractional_scale_manager_v1_get_fractional_scale(
        wl->fractional, native->surface);
    wp_fractional_scale_v1_add_listener(native->fractional, &scale_listener,
                                        window);
  }
  return true;
}

int64_t sw_wayland_current_monitor(SWWindow *window) {
  SWOutput *best = NULL;
  for (SWOutputRef *ref = window->wl->outputs; ref; ref = ref->next)
    if (!best || ref->output->scale > best->scale)
      best = ref->output;
  return best ? best->monitor->data.key : 0;
}

void sw_wayland_destroy_window(SWWindow *window) {
  SWWLWindow *native = window->wl;
  sw_wayland_input_destroy(window);
  sw_wayland_drop_window_destroy(window);
  if (native->frame_callback)
    wl_callback_destroy(native->frame_callback);
  if (native->activation_token)
    xdg_activation_token_v1_destroy(native->activation_token);
  destroy_role(window);
  if (native->fractional)
    wp_fractional_scale_v1_destroy(native->fractional);
  if (native->viewport)
    wp_viewport_destroy(native->viewport);
  wl_surface_destroy(native->surface);
  while (native->outputs) {
    SWOutputRef *ref = native->outputs;
    native->outputs = ref->next;
    free(ref);
  }
  free(native);
  window->wl = NULL;
}
