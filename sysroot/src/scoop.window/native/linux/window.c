#include "window.h"

int sw_action(SWWindow *window, enum SWAction action, double a, double b,
              double c, double d, const char *text) {
  if (window->app->disconnected)
    return sw_error(window->app, SWErrorDisconnected, EPIPE);
  return window->wl ? sw_wayland_action(window, action, a, b, c, d, text)
                    : sw_x11_action(window, action, a, b, c, d, text);
}

static void free_window(SWWindow *window) {
  free(window->title);
  free(window->app_id);
  free(window->instance);
  free(window->surrounding);
  free(window);
}

void *scoop_window_create(void *pointer, int64_t identity, const uint8_t *title,
                          int64_t length, double width, double height,
                          int32_t flags) {
  SWApp *app = pointer;
  if (app->disconnected) {
    sw_error(app, SWErrorDisconnected, EPIPE);
    return NULL;
  }
  SWWindow *window = calloc(1, sizeof(*window));
  if (!window) {
    sw_error(app, SWErrorNative, ENOMEM);
    return NULL;
  }
  window->app = app;
  window->identity = identity;
  window->requested_width = width;
  window->requested_height = height;
  window->max_width = window->max_height = -1;
  window->increment_width = window->increment_height = 1;
  window->snapshot.width = width;
  window->snapshot.height = height;
  window->snapshot.scale = 1;
  window->snapshot.generation = 1;
  window->snapshot.dark = window->snapshot.minimized = -1;
  window->decorations = (flags & 1) != 0;
  window->resizable = (flags & 2) != 0;
  window->transparent = (flags & 4) != 0;
  window->cursor_visible = true;
  window->close_button = window->minimize_button = window->maximize_button =
      true;
  window->title = sw_string(title, length);
  window->app_id = strdup("scoop");
  window->instance = strdup("scoop");
  window->text_revision = -1;
  if (!window->title || !window->app_id || !window->instance) {
    sw_error(app, SWErrorNative, errno);
    free_window(window);
    return NULL;
  }
  window->next = app->windows;
  app->windows = window;
  bool success =
      app->wayland ? sw_wayland_create(window) : sw_x11_create(window);
  if (!success) {
    app->windows = window->next;
    if (window->wl)
      sw_wayland_destroy_window(window);
    if (window->x)
      sw_x11_destroy_window(window);
    free_window(window);
    return NULL;
  }
  return window;
}

SWSnapshot scoop_window_snapshot(void *pointer) {
  return ((SWWindow *)pointer)->snapshot;
}
void scoop_window_destroy(void *pointer) {
  SWWindow *window = pointer;
  SWWindow **link = &window->app->windows;
  while (*link != window)
    link = &(*link)->next;
  *link = window->next;
  if (window->wl)
    sw_wayland_destroy_window(window);
  if (window->x)
    sw_x11_destroy_window(window);
  free_window(window);
}

int32_t scoop_window_title(void *pointer, const uint8_t *text, int64_t length) {
  SWWindow *window = pointer;
  char *value = sw_string(text, length);
  if (!value)
    return sw_error(window->app, SWErrorNative, errno);
  int result = sw_action(window, SWSetTitle, 0, 0, 0, 0, value);
  if (result >= 0 && result != SWUnsupported) {
    free(window->title);
    window->title = value;
  } else
    free(value);
  return result;
}

#define SW_ONE(name, action)                                                   \
  int32_t name(void *pointer, int32_t value) {                                 \
    return sw_action(pointer, action, value, 0, 0, 0, NULL);                   \
  }
SW_ONE(scoop_window_visible, SWSetVisible)
SW_ONE(scoop_window_resizable, SWResizable)
SW_ONE(scoop_window_decorations, SWDecorations)
SW_ONE(scoop_window_level, SWLevel)
SW_ONE(scoop_window_minimized, SWMinimized)
SW_ONE(scoop_window_maximized, SWMaximized)
SW_ONE(scoop_window_theme, SWSetTheme)
SW_ONE(scoop_window_transparent, SWTransparent)
SW_ONE(scoop_window_blur, SWBlur)
SW_ONE(scoop_window_content_protection, SWContentProtection)
SW_ONE(scoop_window_attention, SWAttention)
SW_ONE(scoop_window_cursor_icon, SWCursorIcon)
SW_ONE(scoop_window_cursor_visible, SWCursorVisible)
SW_ONE(scoop_window_ime_allowed, SWImeAllowed)
#undef SW_ONE

int32_t scoop_window_size(void *pointer, double width, double height) {
  return sw_action(pointer, SWSetSize, width, height, 0, 0, NULL);
}
int32_t scoop_window_position(void *pointer, double x, double y) {
  return sw_action(pointer, SWPosition, x, y, 0, 0, NULL);
}
int32_t scoop_window_constraints(void *pointer, double a, double b, double c,
                                 double d) {
  return sw_action(pointer, SWConstraints, a, b, c, d, NULL);
}
int32_t scoop_window_resize_increments(void *pointer, double width,
                                       double height) {
  return sw_action(pointer, SWIncrements, width, height, 0, 0, NULL);
}
int32_t scoop_window_buttons(void *pointer, int32_t close, int32_t minimize,
                             int32_t maximize) {
  return sw_action(pointer, SWButtons, close, minimize, maximize, 0, NULL);
}
int32_t scoop_window_focus(void *pointer) {
  return sw_action(pointer, SWFocus, 0, 0, 0, 0, NULL);
}
int32_t scoop_window_pre_present(void *pointer) {
  return sw_action(pointer, SWPrePresent, 0, 0, 0, 0, NULL);
}
int32_t scoop_window_ime_caret(void *pointer, double x, double y, double width,
                               double height) {
  return sw_action(pointer, SWImeCaret, x, y, width, height, NULL);
}

int32_t scoop_window_linux_text_state(void *pointer, const uint8_t *bytes,
                                      int64_t length, int32_t anchor,
                                      int32_t cursor, int64_t revision,
                                      double x, double y, double width,
                                      double height) {
  SWWindow *window = pointer;
  if (!window->wl)
    return SWUnsupported;
  int result = sw_action(window, SWImeCaret, x, y, width, height, NULL);
  if (result < 0 || result == SWUnsupported)
    return result;
  char *text = sw_string(bytes, length);
  if (!text)
    return sw_error(window->app, SWErrorNative, errno);
  free(window->surrounding);
  window->surrounding = text;
  window->selection_anchor = anchor;
  window->selection_cursor = cursor;
  window->text_revision = revision;
  return sw_action(window, SWSurrounding, 0, 0, 0, 0, NULL);
}

int32_t scoop_window_linux_identity(void *pointer, const uint8_t *id,
                                    int64_t id_length, const uint8_t *instance,
                                    int64_t instance_length) {
  SWWindow *window = pointer;
  char *app_id = sw_string(id, id_length);
  char *app_instance = sw_string(instance, instance_length);
  if (!app_id || !app_instance) {
    free(app_id);
    free(app_instance);
    return sw_error(window->app, SWErrorNative, errno);
  }
  free(window->app_id);
  free(window->instance);
  window->app_id = app_id;
  window->instance = app_instance;
  return sw_action(window, SWAppId, 0, 0, 0, 0, NULL);
}

typedef struct {
  void *display, *surface;
  uint64_t window_id, visual_id;
  int32_t screen, backend;
} SWNativeHandle;
SWNativeHandle scoop_window_linux_handle(void *pointer) {
  SWWindow *window = pointer;
  if (window->wl)
    return (SWNativeHandle){sw_wayland_display(window->app),
                            sw_wayland_surface(window),
                            0,
                            0,
                            0,
                            SWWaylandBackend};
  return (SWNativeHandle){sw_x11_display(window->app), NULL,
                          sw_x11_window_id(window),    sw_x11_visual_id(window),
                          sw_x11_screen(window),       SWX11Backend};
}

int64_t scoop_window_current_monitor(void *pointer) {
  SWWindow *window = pointer;
  return window->wl ? sw_wayland_current_monitor(window)
                    : sw_x11_current_monitor(window);
}
