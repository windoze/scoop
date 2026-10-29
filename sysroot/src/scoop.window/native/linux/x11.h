#ifndef SCOOP_WINDOW_X11_H
#define SCOOP_WINDOW_X11_H

#include "window.h"
#include <X11/XKBlib.h>
#include <X11/Xatom.h>
#include <X11/Xcursor/Xcursor.h>
#include <X11/Xlib.h>
#include <X11/Xutil.h>
#include <X11/extensions/XInput2.h>
#include <X11/extensions/Xrandr.h>
#include <X11/extensions/Xrender.h>
#include <setjmp.h>
#include <signal.h>

typedef struct SWXDevice SWXDevice;

typedef struct {
  sigset_t previous;
  bool consume;
} SWX11Signals;

int sw_x11_signals_begin(SWX11Signals *signals);
void sw_x11_signals_end(SWX11Signals *signals);

struct SWX11 {
  SWApp *app;
  Display *display;
  Window root;
  int screen, xkb_event, randr_event, xi_opcode;
  double scale;
  XIM im;
  XIMStyle im_style;
  SWKeyboardState keyboard;
  SWWindow *focused;
  jmp_buf *trap;
  XErrorHandler previous_error;
  XIOErrorHandler previous_io_error;
  Atom *supported;
  unsigned long supported_count;
  char *locale, *modifiers;
  bool detectable_repeat;
  SWXDevice *devices;
};

struct SWXWindow {
  Window id;
  Visual *visual;
  int depth;
  Colormap colormap;
  XIC ic;
  Cursor cursor, empty_cursor;
  uint32_t *preedit;
  size_t preedit_length;
  int preedit_cursor;
  bool preedit_started;
  SWEvent *text_batch;
  bool text_invalid, text_was_preedit;
  Window drop_source;
  int drop_version;
  bool drop_accepted;
  Atom drop_property;
  SWDrop *drop;
  bool drop_waiting, drop_incremental;
  bool drop_action_copy;
};

// Every scope using Xlib restores its predecessor before returning to Scoop.
#define SW_X11_BEGIN(x, failure)                                               \
  SWX11Signals sw_signals;                                                     \
  int sw_signal_error = sw_x11_signals_begin(&sw_signals);                     \
  if (sw_signal_error) {                                                       \
    sw_error((x)->app, SWErrorNative, sw_signal_error);                        \
    return failure;                                                            \
  }                                                                            \
  jmp_buf sw_trap;                                                             \
  jmp_buf *sw_previous_trap = (x)->trap;                                       \
  if (setjmp(sw_trap)) {                                                       \
    (x)->trap = sw_previous_trap;                                              \
    sw_x11_signals_end(&sw_signals);                                           \
    return failure;                                                            \
  }                                                                            \
  (x)->trap = &sw_trap
#define SW_X11_END(x)                                                          \
  do {                                                                         \
    (x)->trap = sw_previous_trap;                                              \
    sw_x11_signals_end(&sw_signals);                                           \
  } while (0)

Atom sw_xatom(SWX11 *x, const char *name);
bool sw_x11_supported(SWX11 *x, const char *name);
void sw_x11_read_supported(SWX11 *x);
void sw_x11_send(SWWindow *window, const char *name, long a, long b, long c,
                 long d, long e);
SWWindow *sw_x11_find(SWApp *app, Window id);
void sw_x11_state(SWWindow *window);
void sw_x11_keymap(SWX11 *x);
void sw_x11_key(SWWindow *window, XKeyEvent *event, bool repeat);
void sw_x11_input_init(SWWindow *window);
void sw_x11_input_destroy(SWWindow *window);
void sw_x11_input_focus(SWWindow *window, bool focused);
void sw_x11_text_begin(SWWindow *window);
void sw_x11_text_end(SWWindow *window);
int sw_x11_ime(SWWindow *window, bool allowed);
void sw_x11_caret(SWWindow *window);
void sw_x11_monitors(SWX11 *x);
void sw_x11_drop_event(SWWindow *window, XEvent *event);
void sw_x11_drop_destroy(SWWindow *window);
void sw_x11_devices(SWX11 *x);
void sw_x11_devices_free(SWX11 *x);
void sw_x11_pointer_setup(SWWindow *window);
void sw_x11_pointer_event(SWX11 *x, XGenericEventCookie *cookie);
void sw_x11_outer_bounds(SWWindow *window);

#endif
