#include "x11.h"
#include <locale.h>

static SWX11 *active_x11;

static int x_error(Display *display, XErrorEvent *event) {
  SWX11 *x = active_x11;
  if (!x || display != x->display)
    return x && x->previous_error ? x->previous_error(display, event) : 0;
  sw_error(x->app, SWErrorNative, event->error_code);
  x->app->error_pending = true;
  sw_wake(x->app);
  return 0;
}

static int x_io_error(Display *display) {
  SWX11 *x = active_x11;
  if (x && display == x->display && x->trap) {
    sw_disconnect(x->app, errno ? errno : EPIPE);
    longjmp(*x->trap, 1);
  }
  return x && x->previous_io_error ? x->previous_io_error(display) : 0;
}

Atom sw_xatom(SWX11 *x, const char *name) {
  return XInternAtom(x->display, name, False);
}

void sw_x11_read_supported(SWX11 *x) {
  Atom type;
  int format;
  unsigned long count, remaining;
  unsigned char *data = NULL;
  if (XGetWindowProperty(x->display, x->root, sw_xatom(x, "_NET_SUPPORTED"), 0,
                         UINT32_MAX / 4, False, XA_ATOM, &type, &format, &count,
                         &remaining, &data) == Success &&
      type == XA_ATOM && format == 32) {
    if (x->supported)
      XFree(x->supported);
    x->supported = (Atom *)data;
    x->supported_count = count;
  } else if (data)
    XFree(data);
}

bool sw_x11_supported(SWX11 *x, const char *name) {
  Atom atom = sw_xatom(x, name);
  for (unsigned long i = 0; i < x->supported_count; ++i)
    if (x->supported[i] == atom)
      return true;
  return false;
}

SWWindow *sw_x11_find(SWApp *app, Window id) {
  for (SWWindow *window = app->windows; window; window = window->next)
    if (window->x && window->x->id == id)
      return window;
  return NULL;
}

static bool initialize(SWX11 *x) {
  SW_X11_BEGIN(x, false);
  x->screen = DefaultScreen(x->display);
  x->root = RootWindow(x->display, x->screen);
  x->scale = 1;
  XSelectInput(x->display, x->root, PropertyChangeMask);
  int opcode, error, major = XkbMajorVersion, minor = XkbMinorVersion;
  x->xkb_event = -1;
  if (XkbQueryExtension(x->display, &opcode, &x->xkb_event, &error, &major,
                        &minor)) {
    XkbSelectEvents(
        x->display, XkbUseCoreKbd,
        XkbNewKeyboardNotifyMask | XkbMapNotifyMask | XkbStateNotifyMask,
        XkbNewKeyboardNotifyMask | XkbMapNotifyMask | XkbStateNotifyMask);
    Bool supported = False;
    XkbSetDetectableAutoRepeat(x->display, True, &supported);
    x->detectable_repeat = supported;
  }
  x->randr_event = -1;
  if (XRRQueryExtension(x->display, &x->randr_event, &error))
    XRRSelectInput(x->display, x->root,
                   RRScreenChangeNotifyMask | RRCrtcChangeNotifyMask |
                       RROutputChangeNotifyMask | RRResourceChangeNotifyMask);
  x->xi_opcode = -1;
  int first_event;
  if (XQueryExtension(x->display, "XInputExtension", &x->xi_opcode,
                      &first_event, &error)) {
    major = 2;
    minor = 2;
    if (XIQueryVersion(x->display, &major, &minor) != Success || major < 2 ||
        (major == 2 && minor < 1))
      x->xi_opcode = -1;
  }
  x->locale = strdup(setlocale(LC_CTYPE, NULL));
  setlocale(LC_CTYPE, "");
  const char *modifiers = XSetLocaleModifiers(NULL);
  x->modifiers = modifiers ? strdup(modifiers) : NULL;
  XSetLocaleModifiers("");
  x->im = XOpenIM(x->display, NULL, NULL, NULL);
  if (x->im) {
    XIMStyles *styles = NULL;
    if (!XGetIMValues(x->im, XNQueryInputStyle, &styles, NULL) && styles) {
      XIMStyle choices[] = {XIMPreeditCallbacks | XIMStatusNothing,
                            XIMPreeditPosition | XIMStatusNothing,
                            XIMPreeditNothing | XIMStatusNothing,
                            XIMPreeditNone | XIMStatusNone};
      for (size_t c = 0;
           c < sizeof(choices) / sizeof(choices[0]) && !x->im_style; ++c)
        for (unsigned short i = 0; i < styles->count_styles; ++i)
          if (styles->supported_styles[i] == choices[c]) {
            x->im_style = choices[c];
            break;
          }
      XFree(styles);
    }
  }
  sw_x11_keymap(x);
  if (x->xi_opcode >= 0) {
    unsigned char mask[XIMaskLen(XI_LASTEVENT)] = {0};
    XISetMask(mask, XI_HierarchyChanged);
    XISetMask(mask, XI_DeviceChanged);
    XIEventMask selection = {XIAllDevices, sizeof(mask), mask};
    XISelectEvents(x->display, x->root, &selection, 1);
    sw_x11_devices(x);
  }
  sw_x11_read_supported(x);
  sw_x11_monitors(x);
  XFlush(x->display);
  SW_X11_END(x);
  return !x->app->disconnected;
}

bool sw_x11_connect(SWApp *app, const char *display) {
  XInitThreads();
  SWX11 *x = calloc(1, sizeof(*x));
  if (!x) {
    sw_error(app, SWErrorNative, ENOMEM);
    return false;
  }
  x->app = app;
  x->display = XOpenDisplay(display);
  if (!x->display) {
    free(x);
    sw_error(app, SWErrorUnavailable, errno);
    return false;
  }
  app->x11 = x;
  app->display_fd = ConnectionNumber(x->display);
  x->previous_error = XSetErrorHandler(x_error);
  x->previous_io_error = XSetIOErrorHandler(x_io_error);
  active_x11 = x;
  if (!sw_keyboard_init(&x->keyboard) || !initialize(x)) {
    if (!app->failure.kind)
      sw_error(app, SWErrorNative, ENOMEM);
    sw_x11_destroy(app);
    return false;
  }
  return true;
}

static void close_display(SWX11 *x) {
  SW_X11_BEGIN(x, );
  if (x->im && !x->app->disconnected)
    XCloseIM(x->im);
  XCloseDisplay(x->display);
  SW_X11_END(x);
}

void sw_x11_destroy(SWApp *app) {
  SWX11 *x = app->x11;
  if (!x)
    return;
  close_display(x);
  XSetErrorHandler(x->previous_error);
  XSetIOErrorHandler(x->previous_io_error);
  active_x11 = NULL;
  if (x->locale) {
    setlocale(LC_CTYPE, x->locale);
    free(x->locale);
  }
  if (x->modifiers) {
    XSetLocaleModifiers(x->modifiers);
    free(x->modifiers);
  }
  if (x->supported)
    XFree(x->supported);
  sw_keyboard_destroy(&x->keyboard);
  sw_x11_devices_free(x);
  free(x);
  app->x11 = NULL;
}

static void dispatch_window(SWWindow *window, XEvent *event) {
  SWX11 *x = window->app->x11;
  SWSnapshot *value = &window->snapshot;
  switch (event->type) {
  case ClientMessage:
    if (event->xclient.message_type == sw_xatom(x, "WM_PROTOCOLS") &&
        (Atom)event->xclient.data.l[0] == sw_xatom(x, "WM_DELETE_WINDOW"))
      sw_emit(window, SWClose, 0);
    else
      sw_x11_drop_event(window, event);
    break;
  case ConfigureNotify: {
    value->width = event->xconfigure.width / value->scale;
    value->height = event->xconfigure.height / value->scale;
    sw_geometry(window);
    sw_x11_outer_bounds(window);
    sw_emit(window, SWMoved, 0);
    break;
  }
  case MapNotify:
    value->visible = 1;
    value->occluded = 0;
    value->redraw_ready = 1;
    sw_geometry(window);
    break;
  case UnmapNotify:
    value->visible = 0;
    value->redraw_ready = 0;
    sw_x11_state(window);
    break;
  case Expose:
    if (!event->xexpose.count)
      sw_emit(window, SWRedraw, 0);
    break;
  case VisibilityNotify:
    value->occluded = event->xvisibility.state == VisibilityFullyObscured;
    value->redraw_ready = value->visible && !value->occluded;
    sw_emit(window, SWOccluded, value->occluded);
    break;
  case FocusIn:
  case FocusOut:
    if (event->xfocus.mode == NotifyGrab ||
        event->xfocus.mode == NotifyUngrab ||
        event->xfocus.detail == NotifyInferior)
      break;
    value->focused = event->type == FocusIn;
    sw_x11_input_focus(window, value->focused);
    sw_emit(window, SWFocused, value->focused);
    break;
  case KeyPress:
  case KeyRelease: {
    bool repeat = false;
    if (event->type == KeyRelease && !x->detectable_repeat &&
        XPending(x->display)) {
      XEvent next;
      XPeekEvent(x->display, &next);
      if (next.type == KeyPress && next.xkey.keycode == event->xkey.keycode &&
          next.xkey.time == event->xkey.time)
        return;
    }
    for (SWPressedKey *key = x->keyboard.pressed; key; key = key->next)
      if (key->code == event->xkey.keycode) {
        repeat = event->type == KeyPress;
        break;
      }
    sw_x11_key(window, &event->xkey, repeat);
    break;
  }
  case EnterNotify:
  case LeaveNotify:
    sw_pointer(window, event->type == EnterNotify ? 0 : 1,
               event->xcrossing.x / value->scale,
               event->xcrossing.y / value->scale, x->keyboard.modifiers, 0, 0,
               0, 0);
    break;
  case MotionNotify:
    sw_pointer(window, 2, event->xmotion.x / value->scale,
               event->xmotion.y / value->scale, x->keyboard.modifiers, 0, 0, 0,
               0);
    break;
  case ButtonPress:
  case ButtonRelease: {
    unsigned int button = event->xbutton.button;
    double px = event->xbutton.x / value->scale,
           py = event->xbutton.y / value->scale;
    if (button >= 4 && button <= 7) {
      if (event->type == ButtonPress)
        sw_pointer(window, 5, px, py, x->keyboard.modifiers, 0,
                   button == 6   ? 1
                   : button == 7 ? -1
                                 : 0,
                   button == 4   ? 1
                   : button == 5 ? -1
                                 : 0,
                   0);
    } else {
      int mapped = button == 1   ? 0
                   : button == 3 ? 1
                   : button == 2 ? 2
                                 : (int)button;
      sw_pointer(window, event->type == ButtonPress ? 3 : 4, px, py,
                 x->keyboard.modifiers, mapped, 0, 0, 0);
    }
    break;
  }
  case PropertyNotify:
    if (event->xproperty.atom == sw_xatom(x, "_NET_WM_STATE") ||
        event->xproperty.atom == sw_xatom(x, "WM_STATE"))
      sw_x11_state(window);
    if (event->xproperty.atom == sw_xatom(x, "_NET_FRAME_EXTENTS")) {
      sw_x11_outer_bounds(window);
      sw_emit(window, SWMoved, 0);
    }
    sw_x11_drop_event(window, event);
    break;
  case SelectionNotify:
    sw_x11_drop_event(window, event);
    break;
  case DestroyNotify:
    window->x->id = 0;
    sw_error(window->app, SWErrorDisconnected, BadWindow);
    window->app->error_pending = true;
    break;
  default:
    break;
  }
}

void sw_x11_dispatch(SWApp *app) {
  SWX11 *x = app->x11;
  SW_X11_BEGIN(x, );
  for (int i = 0; i < 64 && XPending(x->display); ++i) {
    XEvent event;
    XNextEvent(x->display, &event);
    if (event.type == GenericEvent && event.xcookie.extension == x->xi_opcode &&
        XGetEventData(x->display, &event.xcookie)) {
      sw_x11_pointer_event(x, &event.xcookie);
      XFreeEventData(x->display, &event.xcookie);
      continue;
    }
    SWWindow *window = sw_x11_find(app, event.xany.window);
    SWWindow *text_window = window ? window : x->focused;
    if (text_window)
      sw_x11_text_begin(text_window);
    if ((!window || window->ime_allowed) && XFilterEvent(&event, None)) {
      if (text_window)
        sw_x11_text_end(text_window);
      continue;
    }
    if (event.type == x->xkb_event) {
      XkbEvent *keyboard = (XkbEvent *)&event;
      if (keyboard->any.xkb_type == XkbMapNotify ||
          keyboard->any.xkb_type == XkbNewKeyboardNotify)
        sw_x11_keymap(x);
      else if (keyboard->any.xkb_type == XkbStateNotify && x->keyboard.state) {
        XkbStateNotifyEvent *state = &keyboard->state;
        xkb_state_update_mask(x->keyboard.state, state->base_mods,
                              state->latched_mods, state->locked_mods,
                              state->base_group, state->latched_group,
                              state->locked_group);
        uint64_t modifiers = sw_keyboard_modifiers(&x->keyboard);
        if (modifiers != x->keyboard.modifiers && x->focused) {
          x->keyboard.modifiers = modifiers;
          SWEvent *changed = sw_event(x->focused, SWModifiers);
          if (changed) {
            changed->data.a = (int64_t)modifiers;
            sw_enqueue(app, changed);
          }
        }
      }
    } else if (event.type == x->randr_event ||
               event.type == x->randr_event + RRNotify) {
      XRRUpdateConfiguration(&event);
      sw_x11_monitors(x);
    } else if (event.type == PropertyNotify &&
               event.xproperty.window == x->root) {
      if (event.xproperty.atom == sw_xatom(x, "_NET_SUPPORTED"))
        sw_x11_read_supported(x);
      if (event.xproperty.atom == XA_RESOURCE_MANAGER)
        sw_x11_monitors(x);
    } else if (window)
      dispatch_window(window, &event);
    if (text_window)
      sw_x11_text_end(text_window);
  }
  if (XPending(x->display))
    app->pump = true;
  XFlush(x->display);
  SW_X11_END(x);
}

void *sw_x11_display(SWApp *app) { return app->x11->display; }
uint64_t sw_x11_window_id(SWWindow *window) { return window->x->id; }
uint64_t sw_x11_visual_id(SWWindow *window) {
  return XVisualIDFromVisual(window->x->visual);
}
int sw_x11_screen(SWWindow *window) { return window->app->x11->screen; }
