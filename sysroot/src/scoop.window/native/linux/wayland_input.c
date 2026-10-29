#include "wayland.h"
#include <sys/mman.h>
#include <sys/stat.h>

static void pointer_frame(void *data, struct wl_pointer *pointer);

void sw_wayland_cursor(SWSeat *seat) {
  SWWindow *window = seat->pointer_window;
  if (!window || !seat->pointer)
    return;
  if (!window->cursor_visible) {
    wl_pointer_set_cursor(seat->pointer, seat->pointer_serial, NULL, 0, 0);
    return;
  }
  int scale = (int)ceil(window->snapshot.scale);
  if (!seat->cursor_theme || seat->cursor_scale != scale) {
    struct wl_cursor_theme *theme =
        wl_cursor_theme_load(NULL, 24 * scale, seat->wayland->shm);
    if (!theme) {
      sw_error(window->app, SWErrorUnavailable, ENOENT);
      window->app->error_pending = true;
      return;
    }
    if (seat->cursor_theme)
      wl_cursor_theme_destroy(seat->cursor_theme);
    seat->cursor_theme = theme;
    seat->cursor_scale = scale;
  }
  static const char *names[] = {"default",     "text",      "crosshair",
                                "pointer",     "grab",      "grabbing",
                                "not-allowed", "ew-resize", "ns-resize"};
  struct wl_cursor *cursor = wl_cursor_theme_get_cursor(
      seat->cursor_theme, names[window->cursor_icon]);
  if (!cursor)
    cursor = wl_cursor_theme_get_cursor(seat->cursor_theme, "left_ptr");
  if (!cursor || !cursor->image_count) {
    sw_error(window->app, SWErrorUnavailable, ENOENT);
    window->app->error_pending = true;
    return;
  }
  if (!seat->cursor_surface)
    seat->cursor_surface =
        wl_compositor_create_surface(seat->wayland->compositor);
  struct wl_cursor_image *image = cursor->images[0];
  wl_pointer_set_cursor(seat->pointer, seat->pointer_serial,
                        seat->cursor_surface, (int)image->hotspot_x / scale,
                        (int)image->hotspot_y / scale);
  wl_surface_set_buffer_scale(seat->cursor_surface, scale);
  wl_surface_attach(seat->cursor_surface, wl_cursor_image_get_buffer(image), 0,
                    0);
  wl_surface_damage_buffer(seat->cursor_surface, 0, 0, INT_MAX, INT_MAX);
  wl_surface_commit(seat->cursor_surface);
}

static void pointer_enter(void *data, struct wl_pointer *pointer,
                          uint32_t serial, struct wl_surface *surface,
                          wl_fixed_t x, wl_fixed_t y) {
  SWSeat *seat = data;
  seat->pointer_window = sw_wayland_find(seat->wayland, surface);
  seat->pointer_serial = serial;
  seat->pointer_x = wl_fixed_to_double(x);
  seat->pointer_y = wl_fixed_to_double(y);
  if (seat->pointer_window) {
    sw_wayland_cursor(seat);
    sw_pointer(seat->pointer_window, 0, seat->pointer_x, seat->pointer_y,
               seat->keys.modifiers, 0, 0, 0, 0);
  }
}
static void pointer_leave(void *data, struct wl_pointer *pointer,
                          uint32_t serial, struct wl_surface *surface) {
  SWSeat *seat = data;
  if (seat->pointer_window)
    sw_pointer(seat->pointer_window, 1, seat->pointer_x, seat->pointer_y,
               seat->keys.modifiers, 0, 0, 0, 0);
  seat->pointer_window = NULL;
  seat->scrolling = false;
}
static void pointer_motion(void *data, struct wl_pointer *pointer,
                           uint32_t time, wl_fixed_t x, wl_fixed_t y) {
  SWSeat *seat = data;
  seat->pointer_x = wl_fixed_to_double(x);
  seat->pointer_y = wl_fixed_to_double(y);
  if (seat->pointer_window)
    sw_pointer(seat->pointer_window, 2, seat->pointer_x, seat->pointer_y,
               seat->keys.modifiers, 0, 0, 0, 0);
}
static void pointer_button(void *data, struct wl_pointer *pointer,
                           uint32_t serial, uint32_t time, uint32_t button,
                           uint32_t state) {
  SWSeat *seat = data;
  seat->input_serial = serial;
  if (!seat->pointer_window)
    return;
  int64_t mapped = button == 0x110   ? 0
                   : button == 0x111 ? 1
                   : button == 0x112 ? 2
                                     : button;
  sw_pointer(seat->pointer_window,
             state == WL_POINTER_BUTTON_STATE_PRESSED ? 3 : 4, seat->pointer_x,
             seat->pointer_y, seat->keys.modifiers, mapped, 0, 0, 0);
}
static void pointer_axis(void *data, struct wl_pointer *pointer, uint32_t time,
                         uint32_t axis, wl_fixed_t value) {
  SWSeat *seat = data;
  if (axis > 1)
    return;
  seat->axis[axis] += wl_fixed_to_double(value);
  seat->has_axis[axis] = true;
  if (wl_pointer_get_version(pointer) < 5)
    pointer_frame(data, pointer);
}
static void pointer_source(void *data, struct wl_pointer *pointer,
                           uint32_t source) {
  ((SWSeat *)data)->axis_source = source;
}
static void pointer_stop(void *data, struct wl_pointer *pointer, uint32_t time,
                         uint32_t axis) {
  if (axis <= 1)
    ((SWSeat *)data)->stopped[axis] = true;
}
static void pointer_discrete(void *data, struct wl_pointer *pointer,
                             uint32_t axis, int32_t discrete) {
  SWSeat *seat = data;
  if (axis <= 1) {
    seat->steps[axis] += discrete;
    seat->has_steps[axis] = true;
  }
}
static void pointer_value120(void *data, struct wl_pointer *pointer,
                             uint32_t axis, int32_t value) {
  SWSeat *seat = data;
  if (axis <= 1) {
    seat->steps[axis] += value / 120.0;
    seat->has_steps[axis] = true;
  }
}
static void pointer_frame(void *data, struct wl_pointer *pointer) {
  SWSeat *seat = data;
  SWWindow *window = seat->pointer_window;
  bool lines = seat->has_steps[0] || seat->has_steps[1];
  bool moving = seat->has_axis[0] || seat->has_axis[1] || lines;
  bool stopped = seat->stopped[0] || seat->stopped[1];
  if (window && (moving || stopped)) {
    int phase = 0;
    if (seat->axis_source == WL_POINTER_AXIS_SOURCE_FINGER) {
      phase = stopped && !moving ? 8 : seat->scrolling ? 4 : 1;
      seat->scrolling = moving;
    }
    double dx =
        lines ? -seat->steps[1] : -seat->axis[1] * window->snapshot.scale;
    double dy =
        lines ? -seat->steps[0] : -seat->axis[0] * window->snapshot.scale;
    sw_pointer(window, lines ? 5 : 6, seat->pointer_x, seat->pointer_y,
               seat->keys.modifiers, 0, dx, dy, phase);
  }
  memset(seat->axis, 0, sizeof(seat->axis));
  memset(seat->steps, 0, sizeof(seat->steps));
  memset(seat->has_axis, 0, sizeof(seat->has_axis));
  memset(seat->has_steps, 0, sizeof(seat->has_steps));
  memset(seat->stopped, 0, sizeof(seat->stopped));
}
static const struct wl_pointer_listener pointer_listener = {
    .enter = pointer_enter,
    .leave = pointer_leave,
    .motion = pointer_motion,
    .button = pointer_button,
    .axis = pointer_axis,
    .frame = pointer_frame,
    .axis_source = pointer_source,
    .axis_stop = pointer_stop,
    .axis_discrete = pointer_discrete,
    .axis_value120 = pointer_value120};

static void keyboard_keymap(void *data, struct wl_keyboard *keyboard,
                            uint32_t format, int32_t fd, uint32_t size) {
  SWSeat *seat = data;
  if (format != WL_KEYBOARD_KEYMAP_FORMAT_XKB_V1 || !size) {
    close(fd);
    return;
  }
  struct stat info;
  if (fstat(fd, &info) < 0 || info.st_size < size) {
    close(fd);
    sw_error(seat->wayland->app, SWErrorNative, EINVAL);
    seat->wayland->app->error_pending = true;
    return;
  }
  char *mapping = mmap(NULL, size, PROT_READ, MAP_PRIVATE, fd, 0);
  close(fd);
  if (mapping == MAP_FAILED) {
    sw_error(seat->wayland->app, SWErrorNative, errno);
    seat->wayland->app->error_pending = true;
    return;
  }
  struct xkb_keymap *keymap =
      mapping[size - 1] == 0
          ? xkb_keymap_new_from_string(seat->keys.context, mapping,
                                       XKB_KEYMAP_FORMAT_TEXT_V1,
                                       XKB_KEYMAP_COMPILE_NO_FLAGS)
          : NULL;
  munmap(mapping, size);
  if (!keymap) {
    sw_error(seat->wayland->app, SWErrorNative, EINVAL);
    seat->wayland->app->error_pending = true;
    return;
  }
  struct xkb_state *state = xkb_state_new(keymap);
  if (!state) {
    xkb_keymap_unref(keymap);
    sw_error(seat->wayland->app, SWErrorNative, ENOMEM);
    seat->wayland->app->error_pending = true;
    return;
  }
  if (seat->keyboard_window)
    sw_keyboard_reset(seat->keyboard_window, &seat->keys);
  xkb_state_unref(seat->keys.state);
  xkb_keymap_unref(seat->keys.keymap);
  seat->keys.state = state;
  seat->keys.keymap = keymap;
  seat->repeat_deadline = -1;
}
static void keyboard_enter(void *data, struct wl_keyboard *keyboard,
                           uint32_t serial, struct wl_surface *surface,
                           struct wl_array *keys) {
  SWSeat *seat = data;
  seat->keyboard_window = sw_wayland_find(seat->wayland, surface);
  seat->input_serial = serial;
  seat->repeat_deadline = -1;
  if (!seat->keyboard_window)
    return;
  uint32_t *code;
  if (seat->keys.state)
    wl_array_for_each(code, keys)
        xkb_state_update_key(seat->keys.state, *code + 8, XKB_KEY_DOWN);
  seat->keyboard_window->snapshot.focused = 1;
  sw_emit(seat->keyboard_window, SWFocused, 1);
}
static void keyboard_leave(void *data, struct wl_keyboard *keyboard,
                           uint32_t serial, struct wl_surface *surface) {
  SWSeat *seat = data;
  SWWindow *window = seat->keyboard_window;
  seat->keyboard_window = NULL;
  seat->repeat_deadline = -1;
  if (!window)
    return;
  sw_keyboard_reset(window, &seat->keys);
  bool focused = false;
  for (SWSeat *other = seat->wayland->seats; other; other = other->next)
    if (other->keyboard_window == window)
      focused = true;
  window->snapshot.focused = focused;
  sw_emit(window, SWFocused, focused);
}

static void keyboard_key(void *data, struct wl_keyboard *keyboard,
                         uint32_t serial, uint32_t time, uint32_t key,
                         uint32_t state) {
  SWSeat *seat = data;
  seat->input_serial = serial;
  SWWindow *window = seat->keyboard_window;
  if (!window || !seat->keys.state || key > UINT32_MAX - 8)
    return;
  uint32_t code = key + 8;
  bool pressed = state == WL_KEYBOARD_KEY_STATE_PRESSED;
  // Modifier masks arrive independently; updating keys here would toggle locks
  // twice.
  bool consumed =
      pressed && window->preedit_active && !(seat->keys.modifiers & (2 | 8));
  if (!consumed)
    sw_keyboard_key(window, &seat->keys, code, pressed, false,
                    !seat->text_enabled);
  if (pressed && !consumed && seat->repeat_rate > 0 &&
      xkb_keymap_key_repeats(seat->keys.keymap, code)) {
    seat->repeat_key = code;
    seat->repeat_deadline = sw_now() + (int64_t)seat->repeat_delay * 1000000;
  } else if (!pressed && code == seat->repeat_key)
    seat->repeat_deadline = -1;
}
static void keyboard_modifiers(void *data, struct wl_keyboard *keyboard,
                               uint32_t serial, uint32_t depressed,
                               uint32_t latched, uint32_t locked,
                               uint32_t group) {
  SWSeat *seat = data;
  if (!seat->keys.state)
    return;
  xkb_state_update_mask(seat->keys.state, depressed, latched, locked, 0, 0,
                        group);
  uint64_t flags = sw_keyboard_modifiers(&seat->keys);
  if (flags != seat->keys.modifiers && seat->keyboard_window) {
    SWEvent *event = sw_event(seat->keyboard_window, SWModifiers);
    if (event) {
      event->data.a = (int64_t)flags;
      sw_enqueue(seat->wayland->app, event);
    }
  }
  seat->keys.modifiers = flags;
}
static void keyboard_repeat(void *data, struct wl_keyboard *keyboard,
                            int32_t rate, int32_t delay) {
  SWSeat *seat = data;
  seat->repeat_rate = rate > 0 ? rate : 0;
  seat->repeat_delay = delay > 0 ? delay : 0;
  if (!seat->repeat_rate)
    seat->repeat_deadline = -1;
}
static const struct wl_keyboard_listener keyboard_listener = {
    .keymap = keyboard_keymap,
    .enter = keyboard_enter,
    .leave = keyboard_leave,
    .key = keyboard_key,
    .modifiers = keyboard_modifiers,
    .repeat_info = keyboard_repeat};

static void release_pointer(SWSeat *seat) {
  if (!seat->pointer)
    return;
  pointer_leave(seat, seat->pointer, 0, NULL);
  if (wl_pointer_get_version(seat->pointer) >= 3)
    wl_pointer_release(seat->pointer);
  else
    wl_pointer_destroy(seat->pointer);
  seat->pointer = NULL;
}
static void release_keyboard(SWSeat *seat) {
  if (!seat->keyboard)
    return;
  keyboard_leave(seat, seat->keyboard, 0, NULL);
  if (wl_keyboard_get_version(seat->keyboard) >= 3)
    wl_keyboard_release(seat->keyboard);
  else
    wl_keyboard_destroy(seat->keyboard);
  seat->keyboard = NULL;
}
static void seat_capabilities(void *data, struct wl_seat *native,
                              uint32_t capabilities) {
  SWSeat *seat = data;
  if ((capabilities & WL_SEAT_CAPABILITY_POINTER) && !seat->pointer) {
    seat->pointer = wl_seat_get_pointer(native);
    wl_pointer_add_listener(seat->pointer, &pointer_listener, seat);
  } else if (!(capabilities & WL_SEAT_CAPABILITY_POINTER))
    release_pointer(seat);
  if ((capabilities & WL_SEAT_CAPABILITY_KEYBOARD) && !seat->keyboard) {
    seat->keyboard = wl_seat_get_keyboard(native);
    wl_keyboard_add_listener(seat->keyboard, &keyboard_listener, seat);
  } else if (!(capabilities & WL_SEAT_CAPABILITY_KEYBOARD))
    release_keyboard(seat);
}
static void seat_name(void *data, struct wl_seat *seat, const char *name) {}
static const struct wl_seat_listener seat_listener = {
    .capabilities = seat_capabilities, .name = seat_name};

void sw_wayland_seat(SWWayland *wl, uint32_t name, uint32_t version) {
  SWSeat *seat = calloc(1, sizeof(*seat));
  if (!seat) {
    sw_error(wl->app, SWErrorNative, ENOMEM);
    wl->app->error_pending = true;
    return;
  }
  seat->wayland = wl;
  seat->global = name;
  seat->repeat_deadline = -1;
  seat->text_states_tail = &seat->text_states;
  seat->axis_source = UINT32_MAX;
  if (!sw_keyboard_init(&seat->keys)) {
    free(seat);
    sw_error(wl->app, SWErrorNative, ENOMEM);
    wl->app->error_pending = true;
    return;
  }
  seat->seat = wl_registry_bind(wl->registry, name, &wl_seat_interface,
                                version < 8 ? version : 8);
  seat->next = wl->seats;
  wl->seats = seat;
  wl_seat_add_listener(seat->seat, &seat_listener, seat);
  sw_wayland_text_setup(seat);
  sw_wayland_drop_setup(seat);
}

void sw_wayland_seat_destroy(SWSeat *seat) {
  sw_wayland_text_destroy(seat);
  sw_wayland_drop_destroy(seat);
  release_pointer(seat);
  release_keyboard(seat);
  if (seat->cursor_surface)
    wl_surface_destroy(seat->cursor_surface);
  if (seat->cursor_theme)
    wl_cursor_theme_destroy(seat->cursor_theme);
  sw_keyboard_destroy(&seat->keys);
  if (wl_seat_get_version(seat->seat) >= 5)
    wl_seat_release(seat->seat);
  else
    wl_seat_destroy(seat->seat);
  free(seat);
}

void sw_wayland_input_destroy(SWWindow *window) {
  for (SWSeat *seat = window->app->wayland->seats; seat; seat = seat->next) {
    if (seat->keyboard_window == window)
      keyboard_leave(seat, seat->keyboard, 0, NULL);
    if (seat->pointer_window == window)
      pointer_leave(seat, seat->pointer, 0, NULL);
    if (seat->text_window == window) {
      sw_wayland_text_disable(seat);
      seat->text_window = NULL;
    }
  }
}

int64_t sw_wayland_repeat_deadline(SWApp *app) {
  int64_t deadline = -1;
  for (SWSeat *seat = app->wayland->seats; seat; seat = seat->next)
    if (seat->repeat_deadline >= 0 &&
        (deadline < 0 || seat->repeat_deadline < deadline))
      deadline = seat->repeat_deadline;
  return deadline;
}
void sw_wayland_repeat(SWApp *app) {
  int64_t now = sw_now();
  for (SWSeat *seat = app->wayland->seats; seat; seat = seat->next) {
    if (seat->repeat_deadline < 0 || now < seat->repeat_deadline ||
        !seat->keyboard_window || seat->repeat_rate <= 0)
      continue;
    sw_keyboard_key(seat->keyboard_window, &seat->keys, seat->repeat_key, true,
                    true, !seat->text_enabled);
    seat->repeat_deadline = now + 1000000000 / seat->repeat_rate;
  }
}
