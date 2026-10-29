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

static int dimension(int configured, double requested, double previous,
                     double fallback, double minimum, double maximum) {
  if (configured > 0)
    return configured;
  double value = requested > 0 ? requested : previous > 0 ? previous : fallback;
  value = fmax(minimum, value);
  if (maximum > 0)
    value = fmin(maximum, value);
  return (int)fmax(1, fmin(INT_MAX, ceil(value)));
}

static void configured(SWWindow *window, int width, int height,
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
  width = dimension(width, window->requested_width, window->snapshot.width, 800,
                    window->min_width, window->max_width);
  height = dimension(height, window->requested_height, window->snapshot.height,
                     600, window->min_height, window->max_height);
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
  configured(window, width, height,
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
                          void *data) {}
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
  int width = dimension(native->pending_width, window->requested_width,
                        window->snapshot.width, 800, window->min_width,
                        window->max_width);
  int height = dimension(native->pending_height, window->requested_height,
                         window->snapshot.height, 600, window->min_height,
                         window->max_height);
  xdg_surface_set_window_geometry(surface, 0, 0, width, height);
  configured(window, width, height, (native->pending_states & 1) != 0);
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
                             int32_t width, int32_t height) {}
static void wm_capabilities(void *data, struct xdg_toplevel *toplevel,
                            struct wl_array *capabilities) {}
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

static void limits(SWWindow *window) {
  SWWLWindow *native = window->wl;
  int low_w = (int)ceil(window->min_width),
      low_h = (int)ceil(window->min_height);
  int high_w = window->max_width < 0 ? 0 : (int)ceil(window->max_width);
  int high_h = window->max_height < 0 ? 0 : (int)ceil(window->max_height);
  if (!window->resizable) {
    low_w = high_w = dimension(0, window->requested_width,
                               window->snapshot.width, 800, 0, -1);
    low_h = high_h = dimension(0, window->requested_height,
                               window->snapshot.height, 600, 0, -1);
  }
  if (native->frame) {
    libdecor_frame_set_min_content_size(native->frame, low_w, low_h);
    libdecor_frame_set_max_content_size(native->frame, high_w, high_h);
  } else if (native->toplevel) {
    xdg_toplevel_set_min_size(native->toplevel, low_w, low_h);
    xdg_toplevel_set_max_size(native->toplevel, high_w, high_h);
  }
}

static void buttons(SWWindow *window) {
  if (!window->wl->frame)
    return;
  enum libdecor_capabilities all =
      LIBDECOR_ACTION_MOVE | LIBDECOR_ACTION_RESIZE | LIBDECOR_ACTION_MINIMIZE |
      LIBDECOR_ACTION_CLOSE;
  enum libdecor_capabilities enabled = LIBDECOR_ACTION_MOVE;
  if (window->resizable && window->maximize_button)
    enabled |= LIBDECOR_ACTION_RESIZE;
  if (window->minimize_button)
    enabled |= LIBDECOR_ACTION_MINIMIZE;
  if (window->close_button)
    enabled |= LIBDECOR_ACTION_CLOSE;
  libdecor_frame_unset_capabilities(window->wl->frame, all & ~enabled);
  libdecor_frame_set_capabilities(window->wl->frame, enabled);
}

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
    limits(window);
    buttons(window);
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
    limits(window);
    if (native->requested_maximized)
      xdg_toplevel_set_maximized(native->toplevel);
    wl_surface_commit(native->surface);
  }
  return true;
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

static void frame_done(void *data, struct wl_callback *callback,
                       uint32_t time) {
  SWWindow *window = data;
  wl_callback_destroy(callback);
  window->wl->frame_callback = NULL;
  window->snapshot.redraw_ready =
      window->snapshot.configured && window->snapshot.visible;
  window->app->pump = true;
}
static const struct wl_callback_listener frame_listener = {.done = frame_done};

static void activation_done(void *data, struct xdg_activation_token_v1 *token,
                            const char *value) {
  SWWindow *window = data;
  xdg_activation_v1_activate(window->app->wayland->activation, value,
                             window->wl->surface);
  xdg_activation_token_v1_destroy(token);
  window->wl->activation_token = NULL;
}
static const struct xdg_activation_token_v1_listener activation_listener = {
    .done = activation_done};

static int activate(SWWindow *window) {
  SWWayland *wl = window->app->wayland;
  if (!wl->activation)
    return SWUnsupported;
  if (window->wl->activation_token)
    return SWRequested;
  struct xdg_activation_token_v1 *token =
      xdg_activation_v1_get_activation_token(wl->activation);
  window->wl->activation_token = token;
  xdg_activation_token_v1_add_listener(token, &activation_listener, window);
  xdg_activation_token_v1_set_app_id(token, window->app_id);
  xdg_activation_token_v1_set_surface(token, window->wl->surface);
  for (SWSeat *seat = wl->seats; seat; seat = seat->next) {
    if (seat->input_serial) {
      xdg_activation_token_v1_set_serial(token, seat->input_serial, seat->seat);
      break;
    }
  }
  xdg_activation_token_v1_commit(token);
  return SWRequested;
}

static int resize(SWWindow *window, double width, double height) {
  SWWLWindow *native = window->wl;
  window->requested_width = width;
  window->requested_height = height;
  if (!window->snapshot.configured || window->snapshot.maximized)
    return SWRequested;
  int w = dimension(0, width, window->snapshot.width, 800, window->min_width,
                    window->max_width);
  int h = dimension(0, height, window->snapshot.height, 600, window->min_height,
                    window->max_height);
  if (native->frame) {
    if (!libdecor_frame_is_floating(native->frame))
      return SWRequested;
    struct libdecor_state *state = libdecor_state_new(w, h);
    if (!state)
      return sw_error(window->app, SWErrorNative, ENOMEM);
    libdecor_frame_commit(native->frame, state, NULL);
    libdecor_state_free(state);
  } else if (native->xdg_surface)
    xdg_surface_set_window_geometry(native->xdg_surface, 0, 0, w, h);
  configured(window, w, h, false);
  return SWRequested;
}

int sw_wayland_action(SWWindow *window, enum SWAction op, double a, double b,
                      double c, double d, const char *text) {
  SWWLWindow *native = window->wl;
  SWWayland *wl = window->app->wayland;
  struct xdg_toplevel *top =
      native->frame ? libdecor_frame_get_xdg_toplevel(native->frame)
                    : native->toplevel;
  switch (op) {
  case SWSetTitle:
    if (native->frame)
      libdecor_frame_set_title(native->frame, text);
    else if (top)
      xdg_toplevel_set_title(top, text);
    return SWApplied;
  case SWSetVisible:
    if ((a != 0) == (window->snapshot.visible != 0))
      return SWApplied;
    if (a) {
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
  case SWSetSize:
    return resize(window, a, b);
  case SWPosition:
  case SWIncrements:
  case SWLevel:
  case SWBlur:
  case SWContentProtection:
  case SWSetTheme:
    return SWUnsupported;
  case SWFocus:
    return activate(window);
  case SWConstraints:
    if (a > INT_MAX || b > INT_MAX || c > INT_MAX || d > INT_MAX)
      return sw_error(window->app, SWErrorNative, EOVERFLOW);
    if (c == 0 || d == 0)
      return SWUnsupported;
    window->min_width = a;
    window->min_height = b;
    window->max_width = c;
    window->max_height = d;
    limits(window);
    return resize(window, window->requested_width, window->requested_height);
  case SWResizable:
    window->resizable = a != 0;
    limits(window);
    buttons(window);
    return SWRequested;
  case SWDecorations:
    window->decorations = a != 0;
    if (native->frame) {
      libdecor_frame_set_visibility(native->frame, a != 0);
      return SWApplied;
    }
    if (native->decoration)
      zxdg_toplevel_decoration_v1_set_mode(
          native->decoration, a ? ZXDG_TOPLEVEL_DECORATION_V1_MODE_SERVER_SIDE
                                : ZXDG_TOPLEVEL_DECORATION_V1_MODE_CLIENT_SIDE);
    return SWRequested;
  case SWButtons:
    if (!native->use_decor && (!a || !b || !c))
      return SWUnsupported;
    // libdecor couples the maximize button to its resize capability.
    if (window->resizable && !c)
      return SWUnsupported;
    window->close_button = a != 0;
    window->minimize_button = b != 0;
    window->maximize_button = c != 0;
    buttons(window);
    return SWApplied;
  case SWMinimized:
    if (!a || !top)
      return SWUnsupported;
    xdg_toplevel_set_minimized(top);
    return SWRequested;
  case SWMaximized:
    native->requested_maximized = a != 0;
    if (!top)
      return SWRequested;
    if (native->frame) {
      if (a)
        libdecor_frame_set_maximized(native->frame);
      else
        libdecor_frame_unset_maximized(native->frame);
    } else if (a)
      xdg_toplevel_set_maximized(top);
    else
      xdg_toplevel_unset_maximized(top);
    return SWRequested;
  case SWTransparent:
    window->transparent = a != 0;
    if (a)
      wl_surface_set_opaque_region(native->surface, NULL);
    else {
      struct wl_region *region = wl_compositor_create_region(wl->compositor);
      wl_region_add(region, 0, 0, INT_MAX, INT_MAX);
      wl_surface_set_opaque_region(native->surface, region);
      wl_region_destroy(region);
    }
    return SWApplied;
  case SWAttention:
    if (a < 0) {
      if (!native->activation_token)
        return SWUnsupported;
      xdg_activation_token_v1_destroy(native->activation_token);
      native->activation_token = NULL;
      return SWApplied;
    }
    return activate(window);
  case SWCursorIcon:
  case SWCursorVisible:
    if (op == SWCursorIcon)
      window->cursor_icon = (int)a;
    else
      window->cursor_visible = a != 0;
    for (SWSeat *seat = wl->seats; seat; seat = seat->next)
      if (seat->pointer_window == window)
        sw_wayland_cursor(seat);
    return SWApplied;
  case SWImeAllowed:
    return sw_wayland_ime(window, a != 0);
  case SWImeCaret:
    if (!wl->text_manager)
      return SWUnsupported;
    if (floor(a) < INT32_MIN || floor(a) > INT32_MAX || floor(b) < INT32_MIN ||
        floor(b) > INT32_MAX || ceil(c) > INT32_MAX || ceil(d) > INT32_MAX)
      return sw_error(window->app, SWErrorNative, EOVERFLOW);
    window->caret_x = a;
    window->caret_y = b;
    window->caret_width = c;
    window->caret_height = d;
    for (SWSeat *seat = wl->seats; seat; seat = seat->next)
      if (seat->text_window == window)
        sw_wayland_text_update(seat);
    return SWApplied;
  case SWPrePresent:
    if (!native->frame_callback) {
      native->frame_callback = wl_surface_frame(native->surface);
      wl_callback_add_listener(native->frame_callback, &frame_listener, window);
      window->snapshot.redraw_ready = 0;
    }
    return SWApplied;
  case SWAppId:
    if (native->frame)
      libdecor_frame_set_app_id(native->frame, window->app_id);
    else if (top)
      xdg_toplevel_set_app_id(top, window->app_id);
    return SWApplied;
  case SWSurrounding:
    if (!wl->text_manager)
      return SWUnsupported;
    for (SWSeat *seat = wl->seats; seat; seat = seat->next)
      if (seat->text_window == window)
        sw_wayland_text_update(seat);
    return SWApplied;
  }
  return sw_error(window->app, SWErrorNative, EINVAL);
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
