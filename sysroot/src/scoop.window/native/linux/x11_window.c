#include "x11.h"

void sw_x11_send(SWWindow *window, const char *name, long a, long b, long c,
                 long d, long e) {
  SWX11 *x = window->app->x11;
  XEvent event = {0};
  event.xclient.type = ClientMessage;
  event.xclient.window = window->x->id;
  event.xclient.message_type = sw_xatom(x, name);
  event.xclient.format = 32;
  event.xclient.data.l[0] = a;
  event.xclient.data.l[1] = b;
  event.xclient.data.l[2] = c;
  event.xclient.data.l[3] = d;
  event.xclient.data.l[4] = e;
  XSendEvent(x->display, x->root, False,
             SubstructureRedirectMask | SubstructureNotifyMask, &event);
}

static void title(SWWindow *window, const char *text) {
  SWX11 *x = window->app->x11;
  XStoreName(x->display, window->x->id, text);
  XChangeProperty(x->display, window->x->id, sw_xatom(x, "_NET_WM_NAME"),
                  sw_xatom(x, "UTF8_STRING"), 8, PropModeReplace,
                  (const unsigned char *)text, (int)strlen(text));
}

static int pixels(double value, double scale) {
  double result = ceil(value * scale);
  return result < 1 ? 1 : result > INT_MAX ? INT_MAX : (int)result;
}

static void size_hints(SWWindow *window) {
  SWX11 *x = window->app->x11;
  double scale = window->snapshot.scale;
  XSizeHints hints = {0};
  hints.flags = PMinSize | PResizeInc | PWinGravity;
  hints.win_gravity = NorthWestGravity;
  hints.min_width = pixels(window->min_width, scale);
  hints.min_height = pixels(window->min_height, scale);
  hints.width_inc = pixels(window->increment_width, scale);
  hints.height_inc = pixels(window->increment_height, scale);
  if (window->max_width >= 0 || window->max_height >= 0) {
    hints.flags |= PMaxSize;
    hints.max_width =
        window->max_width < 0 ? INT_MAX : pixels(window->max_width, scale);
    hints.max_height =
        window->max_height < 0 ? INT_MAX : pixels(window->max_height, scale);
  }
  if (!window->resizable) {
    hints.flags |= PMaxSize;
    hints.min_width = hints.max_width = pixels(window->requested_width, scale);
    hints.min_height = hints.max_height =
        pixels(window->requested_height, scale);
  }
  XSetWMNormalHints(x->display, window->x->id, &hints);
}

static void motif(SWWindow *window) {
  SWX11 *x = window->app->x11;
  unsigned long functions = (1u << 2);
  if (window->resizable)
    functions |= 1u << 1;
  if (window->minimize_button)
    functions |= 1u << 3;
  if (window->maximize_button)
    functions |= 1u << 4;
  if (window->close_button)
    functions |= 1u << 5;
  unsigned long hints[5] = {3, functions, window->decorations ? 1u : 0u, 0, 0};
  Atom atom = sw_xatom(x, "_MOTIF_WM_HINTS");
  XChangeProperty(x->display, window->x->id, atom, atom, 32, PropModeReplace,
                  (const unsigned char *)hints, 5);
}

static void identity(SWWindow *window) {
  SWX11 *x = window->app->x11;
  XClassHint hint = {window->instance, window->app_id};
  XSetClassHint(x->display, window->x->id, &hint);
}

static void cursor(SWWindow *window) {
  SWX11 *x = window->app->x11;
  SWXWindow *native = window->x;
  static const char *names[] = {"default",     "text",      "crosshair",
                                "pointer",     "grab",      "grabbing",
                                "not-allowed", "ew-resize", "ns-resize"};
  if (!window->cursor_visible) {
    if (!native->empty_cursor) {
      char bits = 0;
      Pixmap pixmap =
          XCreateBitmapFromData(x->display, native->id, &bits, 1, 1);
      XColor color = {0};
      native->empty_cursor =
          XCreatePixmapCursor(x->display, pixmap, pixmap, &color, &color, 0, 0);
      XFreePixmap(x->display, pixmap);
    }
    XDefineCursor(x->display, native->id, native->empty_cursor);
    return;
  }
  Cursor next =
      XcursorLibraryLoadCursor(x->display, names[window->cursor_icon]);
  XDefineCursor(x->display, native->id, next);
  if (native->cursor)
    XFreeCursor(x->display, native->cursor);
  native->cursor = next;
}

bool sw_x11_create(SWWindow *window) {
  SWX11 *x = window->app->x11;
  SWXWindow *native = calloc(1, sizeof(*native));
  if (!native) {
    sw_error(window->app, SWErrorNative, ENOMEM);
    return false;
  }
  window->x = native;
  SW_X11_BEGIN(x, false);
  native->visual = DefaultVisual(x->display, x->screen);
  native->depth = DefaultDepth(x->display, x->screen);
  XVisualInfo info;
  if (XMatchVisualInfo(x->display, x->screen, 32, TrueColor, &info)) {
    XRenderPictFormat *format =
        XRenderFindVisualFormat(x->display, info.visual);
    if (format && format->type == PictTypeDirect && format->direct.alphaMask) {
      native->visual = info.visual;
      native->depth = info.depth;
    }
  }
  if (window->transparent && native->depth != 32) {
    sw_error(window->app, SWErrorUnsupported, 0);
    SW_X11_END(x);
    return false;
  }
  native->colormap =
      XCreateColormap(x->display, x->root, native->visual, AllocNone);
  XSetWindowAttributes attributes = {0};
  attributes.colormap = native->colormap;
  attributes.border_pixel = 0;
  attributes.background_pixel = window->transparent ? 0 : 0xfffafafa;
  attributes.event_mask = StructureNotifyMask | ExposureMask | FocusChangeMask |
                          VisibilityChangeMask | KeyPressMask | KeyReleaseMask |
                          PropertyChangeMask;
  if (x->xi_opcode < 0)
    attributes.event_mask |= EnterWindowMask | LeaveWindowMask |
                             PointerMotionMask | ButtonPressMask |
                             ButtonReleaseMask;
  window->snapshot.scale = x->scale;
  int width = pixels(window->requested_width, x->scale);
  int height = pixels(window->requested_height, x->scale);
  native->id = XCreateWindow(
      x->display, x->root, 0, 0, (unsigned)width, (unsigned)height, 0,
      native->depth, InputOutput, native->visual,
      CWColormap | CWBorderPixel | CWBackPixel | CWEventMask, &attributes);
  Atom protocols[] = {sw_xatom(x, "WM_DELETE_WINDOW")};
  XSetWMProtocols(x->display, native->id, protocols, 1);
  unsigned long version = 5, pid = (unsigned long)getpid();
  XChangeProperty(x->display, native->id, sw_xatom(x, "XdndAware"), XA_ATOM, 32,
                  PropModeReplace, (const unsigned char *)&version, 1);
  XChangeProperty(x->display, native->id, sw_xatom(x, "_NET_WM_PID"),
                  XA_CARDINAL, 32, PropModeReplace, (const unsigned char *)&pid,
                  1);
  title(window, window->title);
  identity(window);
  size_hints(window);
  motif(window);
  sw_x11_input_init(window);
  sw_x11_pointer_setup(window);
  window->snapshot.width = width / x->scale;
  window->snapshot.height = height / x->scale;
  window->snapshot.outer_width = width;
  window->snapshot.outer_height = height;
  window->snapshot.position_known = 1;
  window->snapshot.configured = 1;
  window->snapshot.minimized = 0;
  window->snapshot.maximized =
      sw_x11_supported(x, "_NET_WM_STATE_MAXIMIZED_HORZ") ? 0 : -1;
  sw_geometry(window);
  XSync(x->display, False);
  bool success = !window->app->error_pending;
  SW_X11_END(x);
  return success;
}

void sw_x11_outer_bounds(SWWindow *window) {
  SWX11 *x = window->app->x11;
  Window child;
  int px, py;
  if (!XTranslateCoordinates(x->display, window->x->id, x->root, 0, 0, &px, &py,
                             &child)) {
    window->snapshot.position_known = 0;
    return;
  }
  unsigned long frame[4] = {0};
  Atom type;
  int format;
  unsigned long count, remaining;
  unsigned char *bytes = NULL;
  if (XGetWindowProperty(x->display, window->x->id,
                         sw_xatom(x, "_NET_FRAME_EXTENTS"), 0, 4, False,
                         XA_CARDINAL, &type, &format, &count, &remaining,
                         &bytes) == Success &&
      type == XA_CARDINAL && format == 32 && count == 4)
    memcpy(frame, bytes, sizeof(frame));
  if (bytes)
    XFree(bytes);
  window->snapshot.position_known = 1;
  window->snapshot.x = px - (double)frame[0];
  window->snapshot.y = py - (double)frame[2];
  window->snapshot.outer_width =
      window->snapshot.pixel_width + (double)frame[0] + frame[1];
  window->snapshot.outer_height =
      window->snapshot.pixel_height + (double)frame[2] + frame[3];
}

void sw_x11_state(SWWindow *window) {
  SWX11 *x = window->app->x11;
  Atom type;
  int format;
  unsigned long count, remaining;
  unsigned char *data = NULL;
  if (XGetWindowProperty(x->display, window->x->id,
                         sw_xatom(x, "_NET_WM_STATE"), 0, UINT32_MAX / 4, False,
                         XA_ATOM, &type, &format, &count, &remaining,
                         &data) == Success &&
      type == XA_ATOM && format == 32) {
    bool horizontal = false, vertical = false;
    for (unsigned long i = 0; i < count; ++i) {
      Atom state = ((Atom *)data)[i];
      horizontal |= state == sw_xatom(x, "_NET_WM_STATE_MAXIMIZED_HORZ");
      vertical |= state == sw_xatom(x, "_NET_WM_STATE_MAXIMIZED_VERT");
    }
    window->snapshot.maximized = horizontal && vertical;
  }
  if (data)
    XFree(data);
  data = NULL;
  Atom wm_state = sw_xatom(x, "WM_STATE");
  if (XGetWindowProperty(x->display, window->x->id, wm_state, 0, 2, False,
                         wm_state, &type, &format, &count, &remaining,
                         &data) == Success &&
      format == 32 && count >= 1)
    window->snapshot.minimized = ((long *)data)[0] == IconicState;
  if (data)
    XFree(data);
}

static int action(SWWindow *window, enum SWAction op, double a, double b,
                  double c, double d, const char *text) {
  SWX11 *x = window->app->x11;
  Window id = window->x->id;
  switch (op) {
  case SWSetTitle:
    title(window, text);
    return SWApplied;
  case SWSetVisible:
    if (a)
      XMapWindow(x->display, id);
    else {
      window->snapshot.redraw_ready = 0;
      XUnmapWindow(x->display, id);
    }
    return SWRequested;
  case SWSetSize:
    if (a * x->scale > INT_MAX || b * x->scale > INT_MAX)
      return sw_error(window->app, SWErrorNative, EOVERFLOW);
    window->requested_width = a;
    window->requested_height = b;
    size_hints(window);
    XResizeWindow(x->display, id, (unsigned)pixels(a, x->scale),
                  (unsigned)pixels(b, x->scale));
    return SWRequested;
  case SWFocus:
    if (!sw_x11_supported(x, "_NET_ACTIVE_WINDOW"))
      return SWUnsupported;
    sw_x11_send(window, "_NET_ACTIVE_WINDOW", 1, CurrentTime, 0, 0, 0);
    return SWRequested;
  case SWPosition:
    if (a < INT_MIN || a > INT_MAX || b < INT_MIN || b > INT_MAX)
      return sw_error(window->app, SWErrorNative, EOVERFLOW);
    if (sw_x11_supported(x, "_NET_MOVERESIZE_WINDOW"))
      sw_x11_send(window, "_NET_MOVERESIZE_WINDOW",
                  NorthWestGravity | (3 << 8) | (1 << 12), (long)a, (long)b, 0,
                  0);
    else
      XMoveWindow(x->display, id, (int)a, (int)b);
    return SWRequested;
  case SWConstraints:
    window->min_width = a;
    window->min_height = b;
    window->max_width = c;
    window->max_height = d;
    size_hints(window);
    return action(window, SWSetSize,
                  fmax(a, c < 0 ? window->snapshot.width
                                : fmin(c, window->snapshot.width)),
                  fmax(b, d < 0 ? window->snapshot.height
                                : fmin(d, window->snapshot.height)),
                  0, 0, NULL);
  case SWIncrements:
    window->increment_width = a;
    window->increment_height = b;
    size_hints(window);
    return SWRequested;
  case SWResizable:
    window->resizable = a != 0;
    size_hints(window);
    motif(window);
    return SWRequested;
  case SWDecorations:
    window->decorations = a != 0;
    motif(window);
    return SWRequested;
  case SWButtons:
    window->close_button = a != 0;
    window->minimize_button = b != 0;
    window->maximize_button = c != 0;
    motif(window);
    return SWRequested;
  case SWLevel: {
    const char *above = "_NET_WM_STATE_ABOVE", *below = "_NET_WM_STATE_BELOW";
    if (!sw_x11_supported(x, above) || !sw_x11_supported(x, below))
      return SWUnsupported;
    sw_x11_send(window, "_NET_WM_STATE", 0, sw_xatom(x, a > 0 ? below : above),
                0, 1, 0);
    sw_x11_send(window, "_NET_WM_STATE", a == 0 ? 0 : 1,
                sw_xatom(x, a > 0 ? above : below), 0, 1, 0);
    return SWRequested;
  }
  case SWMinimized:
    if (a)
      XIconifyWindow(x->display, id, x->screen);
    else
      XMapWindow(x->display, id);
    return SWRequested;
  case SWMaximized:
    if (!sw_x11_supported(x, "_NET_WM_STATE_MAXIMIZED_HORZ") ||
        !sw_x11_supported(x, "_NET_WM_STATE_MAXIMIZED_VERT"))
      return SWUnsupported;
    sw_x11_send(window, "_NET_WM_STATE", a ? 1 : 0,
                sw_xatom(x, "_NET_WM_STATE_MAXIMIZED_HORZ"),
                sw_xatom(x, "_NET_WM_STATE_MAXIMIZED_VERT"), 1, 0);
    return SWRequested;
  case SWSetTheme:
    if (a < 0)
      XDeleteProperty(x->display, id, sw_xatom(x, "_GTK_THEME_VARIANT"));
    else {
      const char *variant = a ? "dark" : "light";
      XChangeProperty(x->display, id, sw_xatom(x, "_GTK_THEME_VARIANT"),
                      sw_xatom(x, "UTF8_STRING"), 8, PropModeReplace,
                      (const unsigned char *)variant, (int)strlen(variant));
    }
    return SWRequested;
  case SWTransparent:
    if (a && window->x->depth != 32)
      return SWUnsupported;
    window->transparent = a != 0;
    XSetWindowBackground(x->display, id, a ? 0 : 0xfffafafa);
    XClearWindow(x->display, id);
    sw_emit(window, SWRedraw, 0);
    return SWApplied;
  case SWBlur:
    if (!sw_x11_supported(x, "_KDE_NET_WM_BLUR_BEHIND_REGION"))
      return SWUnsupported;
    if (a)
      XChangeProperty(x->display, id,
                      sw_xatom(x, "_KDE_NET_WM_BLUR_BEHIND_REGION"),
                      XA_CARDINAL, 32, PropModeReplace, NULL, 0);
    else
      XDeleteProperty(x->display, id,
                      sw_xatom(x, "_KDE_NET_WM_BLUR_BEHIND_REGION"));
    return SWRequested;
  case SWContentProtection:
    return SWUnsupported;
  case SWAttention: {
    XWMHints *hints = XGetWMHints(x->display, id);
    if (!hints)
      hints = XAllocWMHints();
    if (!hints)
      return sw_error(window->app, SWErrorNative, ENOMEM);
    if (a < 0)
      hints->flags &= ~XUrgencyHint;
    else
      hints->flags |= XUrgencyHint;
    XSetWMHints(x->display, id, hints);
    XFree(hints);
    return SWRequested;
  }
  case SWCursorIcon:
    window->cursor_icon = (int)a;
    cursor(window);
    return SWApplied;
  case SWCursorVisible:
    window->cursor_visible = a != 0;
    cursor(window);
    return SWApplied;
  case SWImeAllowed:
    return sw_x11_ime(window, a != 0);
  case SWImeCaret:
    if (!window->x->ic || !(x->im_style & XIMPreeditPosition))
      return SWUnsupported;
    if (a * window->snapshot.scale < SHRT_MIN ||
        a * window->snapshot.scale > SHRT_MAX ||
        (b + d) * window->snapshot.scale < SHRT_MIN ||
        (b + d) * window->snapshot.scale > SHRT_MAX)
      return sw_error(window->app, SWErrorNative, EOVERFLOW);
    window->caret_x = a;
    window->caret_y = b;
    window->caret_width = c;
    window->caret_height = d;
    sw_x11_caret(window);
    return SWApplied;
  case SWPrePresent:
    return SWApplied;
  case SWAppId:
    identity(window);
    return SWApplied;
  case SWSurrounding:
    return SWUnsupported;
  }
  return sw_error(window->app, SWErrorNative, EINVAL);
}

int sw_x11_action(SWWindow *window, enum SWAction op, double a, double b,
                  double c, double d, const char *text) {
  SWX11 *x = window->app->x11;
  SW_X11_BEGIN(x, SWFailed);
  int result = action(window, op, a, b, c, d, text);
  XFlush(x->display);
  SW_X11_END(x);
  return result;
}

static void destroy_native(SWWindow *window) {
  SWX11 *x = window->app->x11;
  SW_X11_BEGIN(x, );
  sw_x11_drop_destroy(window);
  sw_x11_input_destroy(window);
  if (window->x->cursor)
    XFreeCursor(x->display, window->x->cursor);
  if (window->x->empty_cursor)
    XFreeCursor(x->display, window->x->empty_cursor);
  if (window->x->id)
    XDestroyWindow(x->display, window->x->id);
  if (window->x->colormap)
    XFreeColormap(x->display, window->x->colormap);
  XFlush(x->display);
  SW_X11_END(x);
}

void sw_x11_destroy_window(SWWindow *window) {
  SWX11 *x = window->app->x11;
  if (x->focused == window) {
    sw_keyboard_reset(window, &x->keyboard);
    x->focused = NULL;
  }
  if (!window->app->disconnected)
    destroy_native(window);
  sw_drop_free(window->x->drop);
  if (window->x->text_batch)
    sw_event_free(window->x->text_batch);
  free(window->x->preedit);
  free(window->x);
  window->x = NULL;
}
