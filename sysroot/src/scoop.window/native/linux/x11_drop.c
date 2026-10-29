#include "x11.h"

static void reply(SWWindow *window, const char *message, long a, long b, long c,
                  long d) {
  SWX11 *x = window->app->x11;
  SWXWindow *native = window->x;
  if (!native->drop_source || window->app->disconnected)
    return;
  XEvent event = {0};
  event.xclient.type = ClientMessage;
  event.xclient.display = x->display;
  event.xclient.window = native->drop_source;
  event.xclient.message_type = sw_xatom(x, message);
  event.xclient.format = 32;
  event.xclient.data.l[0] = (long)native->id;
  event.xclient.data.l[1] = a;
  event.xclient.data.l[2] = b;
  event.xclient.data.l[3] = c;
  event.xclient.data.l[4] = d;
  XSendEvent(x->display, native->drop_source, False, NoEventMask, &event);
  XFlush(x->display);
}

static void finish(SWWindow *window, bool success) {
  SWXWindow *native = window->x;
  if (native->drop && native->drop->dropped)
    reply(window, "XdndFinished", success ? 1 : 0,
          success ? (long)sw_xatom(window->app->x11, "XdndActionCopy") : 0, 0,
          0);
  else if (native->drop)
    sw_drop_leave(native->drop);
  sw_drop_free(native->drop);
  native->drop = NULL;
  native->drop_source = 0;
  native->drop_accepted = false;
  native->drop_waiting = native->drop_incremental = false;
  if (native->drop_property && !window->app->disconnected && native->id)
    XDeleteProperty(window->app->x11->display, native->id,
                    native->drop_property);
}

static bool accepts(SWWindow *window, XClientMessageEvent *event) {
  SWX11 *x = window->app->x11;
  Atom uri = sw_xatom(x, "text/uri-list");
  if (!(event->data.l[1] & 1))
    return (Atom)event->data.l[2] == uri || (Atom)event->data.l[3] == uri ||
           (Atom)event->data.l[4] == uri;
  Atom type;
  int format;
  unsigned long count, remaining;
  unsigned char *data = NULL;
  bool accepted = false;
  if (XGetWindowProperty(x->display, window->x->drop_source,
                         sw_xatom(x, "XdndTypeList"), 0, UINT32_MAX / 4, False,
                         XA_ATOM, &type, &format, &count, &remaining,
                         &data) == Success &&
      type == XA_ATOM && format == 32) {
    for (unsigned long i = 0; i < count; ++i)
      if (((Atom *)data)[i] == uri) {
        accepted = true;
        break;
      }
  }
  if (data)
    XFree(data);
  return accepted;
}

static void request(SWWindow *window, Time time) {
  SWXWindow *native = window->x;
  SWX11 *x = window->app->x11;
  if (native->drop_waiting || (native->drop && native->drop->loaded))
    return;
  if (!native->drop)
    native->drop = sw_drop_new(window);
  if (!native->drop)
    return;
  native->drop_property = sw_xatom(x, "SCOOP_WINDOW_XDND");
  native->drop_waiting = true;
  XConvertSelection(x->display, sw_xatom(x, "XdndSelection"),
                    sw_xatom(x, "text/uri-list"), native->drop_property,
                    native->id, time);
  XFlush(x->display);
}

static void receive(SWWindow *window, bool initial) {
  SWXWindow *native = window->x;
  SWX11 *x = window->app->x11;
  Atom type;
  int format;
  unsigned long count, remaining;
  unsigned char *data = NULL;
  // XGetWindowProperty copies the property; INCR delivers subsequent chunks as
  // events.
  int result = XGetWindowProperty(x->display, native->id, native->drop_property,
                                  0, UINT32_MAX / 4, True, AnyPropertyType,
                                  &type, &format, &count, &remaining, &data);
  if (result != Success || remaining) {
    if (data)
      XFree(data);
    finish(window, false);
    return;
  }
  if (initial && type == sw_xatom(x, "INCR") && format == 32 && count == 1) {
    native->drop_incremental = true;
    if (data)
      XFree(data);
    XFlush(x->display);
    return;
  }
  bool valid = type == sw_xatom(x, "text/uri-list") && format == 8;
  if (!valid || !sw_drop_append(native->drop, data, (size_t)count)) {
    if (data)
      XFree(data);
    finish(window, false);
    return;
  }
  if (data)
    XFree(data);
  if (!native->drop_incremental || !count) {
    native->drop_waiting = native->drop_incremental = false;
    sw_drop_complete(native->drop);
    if (native->drop->dropped)
      finish(window, true);
  }
  XFlush(x->display);
}

void sw_x11_drop_event(SWWindow *window, XEvent *event) {
  SWX11 *x = window->app->x11;
  SWXWindow *native = window->x;
  if (event->type == SelectionNotify) {
    if (!native->drop_waiting ||
        event->xselection.selection != sw_xatom(x, "XdndSelection"))
      return;
    if (event->xselection.property != native->drop_property)
      finish(window, false);
    else
      receive(window, true);
    return;
  }
  if (event->type == PropertyNotify) {
    if (native->drop_incremental &&
        event->xproperty.atom == native->drop_property &&
        event->xproperty.state == PropertyNewValue)
      receive(window, false);
    return;
  }
  if (event->type != ClientMessage || event->xclient.format != 32)
    return;
  XClientMessageEvent *message = &event->xclient;
  Atom kind = message->message_type;
  if (kind == sw_xatom(x, "XdndEnter")) {
    finish(window, false);
    native->drop_source = (Window)message->data.l[0];
    native->drop_version =
        (int)((unsigned long)message->data.l[1] >> 24) & 0xff;
    native->drop_accepted =
        native->drop_version >= 3 && accepts(window, message);
  } else if ((Window)message->data.l[0] != native->drop_source)
    return;
  else if (kind == sw_xatom(x, "XdndPosition")) {
    bool accepted = native->drop_accepted &&
                    (Atom)message->data.l[4] == sw_xatom(x, "XdndActionCopy");
    native->drop_action_copy = accepted;
    reply(window, "XdndStatus", accepted ? 3 : 2, 0, 0,
          accepted ? (long)sw_xatom(x, "XdndActionCopy") : 0);
    if (accepted)
      request(window, (Time)message->data.l[3]);
  } else if (kind == sw_xatom(x, "XdndLeave")) {
    if (!native->drop || !native->drop->dropped)
      finish(window, false);
  } else if (kind == sw_xatom(x, "XdndDrop")) {
    if (!native->drop_accepted || !native->drop_action_copy) {
      reply(window, "XdndFinished", 0, 0, 0, 0);
      finish(window, false);
      return;
    }
    request(window, (Time)message->data.l[2]);
    if (native->drop) {
      sw_drop_accept(native->drop);
      if (native->drop->loaded)
        finish(window, true);
    }
  }
}

void sw_x11_drop_destroy(SWWindow *window) { finish(window, false); }
