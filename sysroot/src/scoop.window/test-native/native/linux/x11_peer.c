#include "x11_peer.h"
#include <errno.h>
#include <signal.h>

static const char uri_list[] =
    "file:///tmp/first%20file\r\nfile://localhost/tmp/%FF\r\n";
int32_t scoop_test_x11_sigpipe_blocked(void) {
  sigset_t signals;
  int error = pthread_sigmask(SIG_BLOCK, NULL, &signals);
  return error ? -error : sigismember(&signals, SIGPIPE);
}
static Atom atom(XTServer *server, const char *name) {
  return XInternAtom(server->display, name, False);
}
static void message(XTServer *server, const char *name, long a, long b, long c,
                    long d, long e) {
  XEvent event = {0};
  event.xclient.type = ClientMessage;
  event.xclient.window = server->target;
  event.xclient.message_type = atom(server, name);
  event.xclient.format = 32;
  event.xclient.data.l[0] = a;
  event.xclient.data.l[1] = b;
  event.xclient.data.l[2] = c;
  event.xclient.data.l[3] = d;
  event.xclient.data.l[4] = e;
  XSendEvent(server->display, server->target, False, NoEventMask, &event);
}
static void key(XTServer *server, KeySym symbol, bool pressed) {
  KeyCode code = XKeysymToKeycode(server->display, symbol);
  if (!code) {
    atomic_fetch_add(&server->errors, 1);
    return;
  }
  XTestFakeKeyEvent(server->display, code, pressed, CurrentTime);
}
static void tap(XTServer *server, KeySym symbol) {
  key(server, symbol, true);
  key(server, symbol, false);
}
static void command(XTServer *server, int value) {
  server->target = (Window)atomic_load(&server->requested_target);
  if (value == 1) {
    XSetInputFocus(server->display, server->target, RevertToParent,
                   CurrentTime);
    XWarpPointer(server->display, None, server->target, 0, 0, 0, 0, 18, 30);
    tap(server, XK_a);
    key(server, XK_Shift_L, true);
    tap(server, XK_a);
    key(server, XK_Shift_L, false);
    key(server, XK_Control_L, true);
    tap(server, XK_c);
    key(server, XK_Control_L, false);
    XTestFakeButtonEvent(server->display, 1, True, CurrentTime);
    XTestFakeButtonEvent(server->display, 1, False, CurrentTime);
    XTestFakeButtonEvent(server->display, 4, True, CurrentTime);
    XTestFakeButtonEvent(server->display, 4, False, CurrentTime);
    key(server, XK_a, true);
    XSetInputFocus(server->display, PointerRoot, RevertToParent, CurrentTime);
    key(server, XK_a, false);
    XResizeWindow(server->display, server->target, 319, 193);
  } else if (value == 2 || value == 3) {
    server->incremental = value == 3;
    server->offset = 0;
    XSetSelectionOwner(server->display, atom(server, "XdndSelection"),
                       server->source, CurrentTime);
    message(server, "XdndEnter", server->source, 5L << 24,
            atom(server, "text/uri-list"), 0, 0);
    message(server, "XdndPosition", server->source, 0, 0, CurrentTime,
            atom(server, "XdndActionCopy"));
  } else if (value == 8) {
    XKillClient(server->display, server->target);
    XSync(server->display, False);
    uint64_t ready = 1;
    if (write(server->killed, &ready, sizeof(ready)) != sizeof(ready))
      atomic_fetch_add(&server->errors, 1);
  } else if (value >= 4 && value <= 7) {
    XSetInputFocus(server->display, server->target, RevertToParent,
                   CurrentTime);
    xt_im_command(server, value);
  } else
    atomic_fetch_add(&server->errors, 1);
  XFlush(server->display);
}
static void selection(XTServer *server, XSelectionRequestEvent *request) {
  server->requestor = request->requestor;
  server->property =
      request->property == None ? request->target : request->property;
  XEvent reply = {0};
  reply.xselection.type = SelectionNotify;
  reply.xselection.requestor = request->requestor;
  reply.xselection.selection = request->selection;
  reply.xselection.target = request->target;
  reply.xselection.property = server->property;
  reply.xselection.time = request->time;
  if (request->target != atom(server, "text/uri-list")) {
    reply.xselection.property = None;
  } else if (server->incremental) {
    unsigned long size = sizeof(uri_list) - 1;
    XSelectInput(server->display, request->requestor, PropertyChangeMask);
    XChangeProperty(server->display, request->requestor, server->property,
                    atom(server, "INCR"), 32, PropModeReplace,
                    (const unsigned char *)&size, 1);
  } else
    XChangeProperty(server->display, request->requestor, server->property,
                    request->target, 8, PropModeReplace,
                    (const unsigned char *)uri_list, sizeof(uri_list) - 1);
  XSendEvent(server->display, request->requestor, False, NoEventMask, &reply);
}
static void event(XTServer *server, XEvent *event) {
  if (event->type == SelectionRequest)
    selection(server, &event->xselectionrequest);
  else if (event->type == PropertyNotify &&
           event->xproperty.window == server->requestor &&
           event->xproperty.atom == server->property &&
           event->xproperty.state == PropertyDelete && server->incremental) {
    size_t remaining = sizeof(uri_list) - 1 - server->offset;
    size_t count = remaining < 9 ? remaining : 9;
    XChangeProperty(server->display, server->requestor, server->property,
                    atom(server, "text/uri-list"), 8, PropModeReplace,
                    (const unsigned char *)uri_list + server->offset,
                    (int)count);
    server->offset += count;
    if (!count)
      server->incremental = false;
  } else if (event->type == ClientMessage) {
    if (event->xclient.message_type == atom(server, "XdndStatus")) {
      if (!(event->xclient.data.l[1] & 1))
        atomic_fetch_add(&server->errors, 1);
      else
        message(server, "XdndDrop", server->source, 0, CurrentTime, 0, 0);
    } else if (event->xclient.message_type == atom(server, "XdndFinished")) {
      if (!(event->xclient.data.l[1] & 1))
        atomic_fetch_add(&server->errors, 1);
      int count = atomic_fetch_add(&server->finished, 1) + 1;
      XResizeWindow(server->display, server->target, (unsigned)(320 + count),
                    193);
    }
  }
}
static void *run(void *pointer) {
  XTServer *server = pointer;
  for (;;) {
    while (XPending(server->display)) {
      XEvent value;
      XNextEvent(server->display, &value);
      event(server, &value);
    }
    if (server->connection) {
      xcb_generic_event_t *value;
      while ((value = xcb_poll_for_event(server->connection))) {
        if (!xcb_im_filter_event(server->im, value) &&
            (value->response_type & 127) == 0)
          atomic_fetch_add(&server->errors, 1);
        free(value);
      }
      xcb_flush(server->connection);
    }
    int value = atomic_exchange(&server->command, 0);
    if (value == -1)
      break;
    if (value)
      command(server, value);
    XFlush(server->display);
    // Atom and input-method queries can read events into Xlib's queue while
    // processing a command. Service those before waiting on the socket again.
    if (XEventsQueued(server->display, QueuedAlready))
      continue;
    struct pollfd fds[3] = {
        {ConnectionNumber(server->display), POLLIN, 0},
        {server->wake, POLLIN, 0},
        {server->connection ? xcb_get_file_descriptor(server->connection) : -1,
         POLLIN, 0}};
    int result;
    do {
      result = poll(fds, 3, -1);
    } while (result < 0 && errno == EINTR);
    if (result < 0) {
      atomic_fetch_add(&server->errors, 1);
      break;
    }
    uint64_t count;
    while (read(server->wake, &count, sizeof(count)) > 0) {
    }
  }
  return NULL;
}
void *scoop_test_x11_peer_start(int32_t ime) {
  XInitThreads();
  XTServer *server = calloc(1, sizeof(*server));
  if (!server)
    return NULL;
  server->wake = server->killed = -1;
  server->display = XOpenDisplay(NULL);
  if (!server->display)
    goto failed;
  server->source = XCreateSimpleWindow(
      server->display, DefaultRootWindow(server->display), 0, 0, 1, 1, 0, 0, 0);
  server->wake = eventfd(0, EFD_CLOEXEC | EFD_NONBLOCK);
  server->killed = eventfd(0, EFD_CLOEXEC);
  if (server->wake < 0 || server->killed < 0 || (ime && !xt_im_start(server)))
    goto failed;
  XFlush(server->display);
  if (pthread_create(&server->thread, NULL, run, server))
    goto failed;
  return server;
failed:
  xt_im_stop(server);
  if (server->wake >= 0)
    close(server->wake);
  if (server->killed >= 0)
    close(server->killed);
  if (server->display)
    XCloseDisplay(server->display);
  free(server);
  return NULL;
}
void scoop_test_x11_peer_command(void *pointer, uint64_t target,
                                 int32_t command) {
  XTServer *server = pointer;
  atomic_store(&server->requested_target, target);
  atomic_store(&server->command, command);
  uint64_t value = 1;
  if (write(server->wake, &value, sizeof(value)) != sizeof(value))
    atomic_fetch_add(&server->errors, 1);
}
int32_t scoop_test_x11_peer_errors(void *pointer) {
  return atomic_load(&((XTServer *)pointer)->errors);
}
void scoop_test_x11_peer_disconnect(void *pointer, uint64_t target) {
  XTServer *server = pointer;
  scoop_test_x11_peer_command(server, target, 8);
  uint64_t value;
  ssize_t count;
  do {
    count = read(server->killed, &value, sizeof(value));
  } while (count < 0 && errno == EINTR);
  if (count != sizeof(value))
    atomic_fetch_add(&server->errors, 1);
}
void scoop_test_x11_peer_stop(void *pointer) {
  XTServer *server = pointer;
  scoop_test_x11_peer_command(server, 0, -1);
  pthread_join(server->thread, NULL);
  xt_im_stop(server);
  close(server->wake);
  close(server->killed);
  XDestroyWindow(server->display, server->source);
  XCloseDisplay(server->display);
  free(server);
}
