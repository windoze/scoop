#ifndef SCOOP_WINDOW_LINUX_H
#define SCOOP_WINDOW_LINUX_H

#ifndef _GNU_SOURCE
#define _GNU_SOURCE
#endif
#include "../abi.h"
#include <errno.h>
#include <limits.h>
#include <math.h>
#include <poll.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>
#include <xkbcommon/xkbcommon-compose.h>
#include <xkbcommon/xkbcommon.h>

typedef struct SWApp SWApp;
typedef struct SWWindow SWWindow;
typedef struct SWEvent SWEvent;
typedef struct SWWatch SWWatch;
typedef struct SWWayland SWWayland;
typedef struct SWX11 SWX11;
typedef struct SWWLWindow SWWLWindow;
typedef struct SWXWindow SWXWindow;
typedef struct SWTransfer SWTransfer;
typedef struct SWDrop SWDrop;

typedef struct {
  int32_t backend, kind, code;
} SWFailure;
enum { SWWaylandBackend = 1, SWX11Backend = 2 };
enum {
  SWErrorUnavailable = 1,
  SWErrorNative = 2,
  SWErrorDisconnected = 3,
  SWErrorUnsupported = 4
};

struct SWEvent {
  SWEventData data;
  SWTextEditData *edits;
  size_t edit_count;
  int64_t revision;
  SWEvent *next;
};
struct SWWatch {
  SWApp *app;
  int64_t identity;
  int fd;
  bool removing;
  SWWatch *next;
};
typedef struct SWMonitor {
  SWMonitorData data;
  uint64_t native_id;
  struct SWMonitor *next;
} SWMonitor;
typedef struct SWPressedKey {
  uint32_t code, symbol;
  char *text;
  struct SWPressedKey *next;
} SWPressedKey;
typedef struct {
  struct xkb_context *context;
  struct xkb_keymap *keymap;
  struct xkb_state *state;
  struct xkb_compose_table *compose_table;
  struct xkb_compose_state *compose;
  SWPressedKey *pressed;
  uint64_t modifiers;
} SWKeyboardState;

struct SWApp {
  SWCallback callback;
  void *context;
  int backend, wake_fd, display_fd;
  bool stopped, disconnected, error_pending, pump;
  int64_t deadline;
  SWFailure failure;
  SWWayland *wayland;
  SWX11 *x11;
  SWWindow *windows;
  SWWatch *watches;
  SWEvent *events, **events_tail;
  SWMonitor *monitors;
  int64_t next_monitor;
  SWTransfer *transfers;
};

struct SWWindow {
  SWApp *app;
  int64_t identity;
  SWSnapshot snapshot;
  double requested_width, requested_height;
  double min_width, min_height, max_width, max_height;
  double increment_width, increment_height;
  char *title, *app_id, *instance;
  bool decorations, resizable, transparent, cursor_visible;
  bool close_button, minimize_button, maximize_button;
  bool ime_allowed, preedit_active;
  int cursor_icon;
  double caret_x, caret_y, caret_width, caret_height;
  char *surrounding;
  int32_t selection_anchor, selection_cursor;
  int64_t text_revision;
  SWWLWindow *wl;
  SWXWindow *x;
  SWWindow *next;
};

enum SWAction {
  SWSetTitle,
  SWSetSize,
  SWSetVisible,
  SWFocus,
  SWPosition,
  SWConstraints,
  SWIncrements,
  SWResizable,
  SWDecorations,
  SWButtons,
  SWLevel,
  SWMinimized,
  SWMaximized,
  SWSetTheme,
  SWTransparent,
  SWBlur,
  SWContentProtection,
  SWAttention,
  SWCursorIcon,
  SWCursorVisible,
  SWImeAllowed,
  SWImeCaret,
  SWPrePresent,
  SWAppId,
  SWSurrounding
};

int64_t sw_now(void);
int sw_error(SWApp *app, int kind, int code);
void sw_disconnect(SWApp *app, int code);
void sw_wake(SWApp *app);
char *sw_string(const uint8_t *text, int64_t length);
SWEvent *sw_event(SWWindow *window, int kind);
void sw_enqueue(SWApp *app, SWEvent *event);
void sw_event_free(SWEvent *event);
void sw_emit(SWWindow *window, int kind, int flags);
void sw_geometry(SWWindow *window);
void sw_text_edit(SWEvent *event, int kind, const char *text, int64_t anchor,
                  int64_t cursor);
void sw_commit(SWWindow *window, const char *text);
void sw_cancel_preedit(SWWindow *window);
void sw_pointer(SWWindow *window, int kind, double x, double y,
                uint64_t modifiers, int64_t button, double dx, double dy,
                int phase);
void sw_monitor_changed(SWApp *app);
void sw_monitors_free(SWMonitor *monitors);
int sw_action(SWWindow *window, enum SWAction action, double a, double b,
              double c, double d, const char *text);

bool sw_keyboard_init(SWKeyboardState *keyboard);
void sw_keyboard_destroy(SWKeyboardState *keyboard);
void sw_keyboard_reset(SWWindow *window, SWKeyboardState *keyboard);
uint64_t sw_keyboard_modifiers(SWKeyboardState *keyboard);
void sw_key(SWWindow *window, SWKeyboardState *keyboard, uint32_t code,
            uint32_t symbol, const char *text, bool pressed, bool repeat,
            bool commit);
void sw_keyboard_key(SWWindow *window, SWKeyboardState *keyboard, uint32_t code,
                     bool pressed, bool repeat, bool commit);

bool sw_wayland_connect(SWApp *app, const char *display);
void sw_wayland_destroy(SWApp *app);
void sw_wayland_dispatch(SWApp *app);
int sw_wayland_prepare(SWApp *app, bool *write);
void sw_wayland_read(SWApp *app, bool readable);
int sw_wayland_extra_fd(SWApp *app);
int64_t sw_wayland_repeat_deadline(SWApp *app);
void sw_wayland_repeat(SWApp *app);
bool sw_wayland_create(SWWindow *window);
void sw_wayland_destroy_window(SWWindow *window);
int sw_wayland_action(SWWindow *window, enum SWAction action, double a,
                      double b, double c, double d, const char *text);
void *sw_wayland_display(SWApp *app);
void *sw_wayland_surface(SWWindow *window);
int64_t sw_wayland_current_monitor(SWWindow *window);

bool sw_x11_connect(SWApp *app, const char *display);
void sw_x11_destroy(SWApp *app);
void sw_x11_dispatch(SWApp *app);
bool sw_x11_create(SWWindow *window);
void sw_x11_destroy_window(SWWindow *window);
int sw_x11_action(SWWindow *window, enum SWAction action, double a, double b,
                  double c, double d, const char *text);
void *sw_x11_display(SWApp *app);
uint64_t sw_x11_window_id(SWWindow *window);
uint64_t sw_x11_visual_id(SWWindow *window);
int sw_x11_screen(SWWindow *window);
int64_t sw_x11_current_monitor(SWWindow *window);

size_t sw_transfer_count(SWApp *app);
void sw_transfer_poll(SWApp *app, struct pollfd *fds);
void sw_transfer_ready(SWApp *app, struct pollfd *fds);
void sw_transfer_destroy(SWApp *app);
void sw_transfer_cancel(SWApp *app, SWDrop *drop);

struct SWDrop {
  SWWindow *window;
  int64_t generation;
  uint8_t *bytes;
  size_t length, capacity;
  bool loaded, entered, dropped;
};
SWDrop *sw_drop_new(SWWindow *window);
bool sw_drop_append(SWDrop *drop, const void *bytes, size_t length);
void sw_drop_complete(SWDrop *drop);
void sw_drop_accept(SWDrop *drop);
void sw_drop_leave(SWDrop *drop);
void sw_drop_free(SWDrop *drop);

#endif
