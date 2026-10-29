#include "wayland.h"

void sw_wayland_limits(SWWindow *window) {
  SWWLWindow *native = window->wl;
  int low_w = (int)ceil(window->min_width),
      low_h = (int)ceil(window->min_height);
  int high_w = window->max_width < 0 ? 0 : (int)ceil(window->max_width);
  int high_h = window->max_height < 0 ? 0 : (int)ceil(window->max_height);
  if (!window->resizable) {
    low_w = high_w = sw_wayland_dimension(0, window->requested_width,
                                          window->snapshot.width, 800, 0, -1);
    low_h = high_h = sw_wayland_dimension(0, window->requested_height,
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

void sw_wayland_buttons(SWWindow *window) {
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
  int w = sw_wayland_dimension(0, width, window->snapshot.width, 800,
                               window->min_width, window->max_width);
  int h = sw_wayland_dimension(0, height, window->snapshot.height, 600,
                               window->min_height, window->max_height);
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
  sw_wayland_configured(window, w, h, false);
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
    return sw_wayland_visibility(window, a != 0);
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
    sw_wayland_limits(window);
    return resize(window, window->requested_width, window->requested_height);
  case SWResizable:
    window->resizable = a != 0;
    sw_wayland_limits(window);
    sw_wayland_buttons(window);
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
    sw_wayland_buttons(window);
    return SWApplied;
  case SWMinimized:
    if (!a || !top || (native->capabilities_known && !native->can_minimize))
      return SWUnsupported;
    xdg_toplevel_set_minimized(top);
    return SWRequested;
  case SWMaximized:
    if (native->capabilities_known && !native->can_maximize)
      return SWUnsupported;
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
  }
  return sw_error(window->app, SWErrorNative, EINVAL);
}
