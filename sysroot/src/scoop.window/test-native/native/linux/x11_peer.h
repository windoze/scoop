#ifndef SCOOP_TEST_X11_PEER_H
#define SCOOP_TEST_X11_PEER_H
#include <X11/Xatom.h>
#include <X11/Xlib.h>
#include <X11/extensions/XTest.h>
#include <X11/keysym.h>
#include <poll.h>
#include <pthread.h>
#include <stdatomic.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/eventfd.h>
#include <unistd.h>
#include <xcb-imdkit/encoding.h>
#include <xcb-imdkit/imdkit.h>

typedef struct {
  Display *display;
  Window source, target, requestor;
  Atom property;
  int wake, killed;
  pthread_t thread;
  atomic_int command, errors, finished;
  atomic_uint_fast64_t requested_target;
  bool incremental;
  size_t offset;
  xcb_connection_t *connection;
  xcb_im_t *im;
  xcb_im_input_context_t *ic;
  int pending_im;
  char *old_modifiers;
} XTServer;

bool xt_im_start(XTServer *server);
void xt_im_command(XTServer *server, int command);
void xt_im_stop(XTServer *server);
#endif
