#include "window.h"
#include <sys/eventfd.h>
#include <sys/syscall.h>

static _Thread_local SWFailure creation_failure;

int32_t scoop_window_is_main_thread(void) {
  return syscall(SYS_gettid) == getpid();
}

void sw_wake(SWApp *app) {
  uint64_t value = 1;
  while (write(app->wake_fd, &value, sizeof(value)) < 0 && errno == EINTR) {
  }
}

void *scoop_window_linux_application_create(SWCallback callback, void *context,
                                            int32_t backend,
                                            const uint8_t *name,
                                            int64_t length) {
  creation_failure = (SWFailure){backend == 2 ? 2 : 1, SWErrorUnavailable, 0};
  char *display = sw_string(name, length);
  if (!display) {
    creation_failure.code = errno;
    return NULL;
  }
  SWApp *app = calloc(1, sizeof(*app));
  if (!app) {
    free(display);
    creation_failure.code = ENOMEM;
    return NULL;
  }
  app->callback = callback;
  app->context = context;
  app->deadline = -1;
  app->display_fd = -1;
  app->events_tail = &app->events;
  app->next_monitor = 1;
  app->wake_fd = eventfd(0, EFD_NONBLOCK | EFD_CLOEXEC);
  if (app->wake_fd < 0) {
    creation_failure.code = errno;
    free(display);
    free(app);
    return NULL;
  }
  bool connected = false;
  if (backend != 2) {
    app->backend = SWWaylandBackend;
    connected = sw_wayland_connect(app, *display ? display : NULL);
  }
  if (!connected && backend != 1) {
    app->disconnected = app->error_pending = false;
    app->backend = SWX11Backend;
    connected = sw_x11_connect(app, *display ? display : NULL);
  }
  free(display);
  if (!connected) {
    creation_failure = app->failure;
    while (app->events) {
      SWEvent *event = app->events;
      app->events = event->next;
      sw_event_free(event);
    }
    sw_monitors_free(app->monitors);
    close(app->wake_fd);
    free(app);
    return NULL;
  }
  app->pump = true;
  return app;
}

SWFailure scoop_window_linux_creation_error(void) { return creation_failure; }
SWFailure scoop_window_linux_error(void *pointer) {
  return ((SWApp *)pointer)->failure;
}
int32_t scoop_window_linux_backend(void *pointer) {
  return ((SWApp *)pointer)->backend;
}

static void retire_watches(SWApp *app) {
  SWWatch **link = &app->watches;
  while (*link) {
    SWWatch *watch = *link;
    if (!watch->removing) {
      link = &watch->next;
      continue;
    }
    *link = watch->next;
    app->callback(app->context, watch->identity, 3);
    free(watch);
  }
}

static int timeout_ms(SWApp *app) {
  int64_t deadline = app->deadline;
  if (app->wayland && !app->disconnected) {
    int64_t repeat = sw_wayland_repeat_deadline(app);
    if (repeat >= 0 && (deadline < 0 || repeat < deadline))
      deadline = repeat;
  }
  if (app->pump || app->events || app->error_pending)
    return 0;
  if (deadline < 0)
    return -1;
  int64_t remaining = deadline - sw_now();
  if (remaining <= 0)
    return 0;
  int64_t millis = remaining / 1000000 + (remaining % 1000000 != 0);
  return millis > INT_MAX ? INT_MAX : (int)millis;
}

int32_t scoop_window_application_run(void *pointer) {
  SWApp *app = pointer;
  while (!app->stopped) {
    if (!app->disconnected) {
      if (app->wayland) {
        sw_wayland_dispatch(app);
        sw_wayland_repeat(app);
      } else
        sw_x11_dispatch(app);
    }
    retire_watches(app);
    if (app->error_pending) {
      app->error_pending = false;
      return SWFailed;
    }
    bool deadline = app->deadline >= 0 && sw_now() >= app->deadline;
    if (deadline)
      app->deadline = -1;
    if (app->pump || app->events || deadline) {
      app->pump = false;
      app->callback(app->context, 0, deadline ? 1 : 0);
    }
    if (app->stopped)
      break;
    // No callbacks run between a successful prepare and its read/cancel pair.
    bool prepared = false, writable = false;
    if (app->wayland && !app->disconnected) {
      int result = sw_wayland_prepare(app, &writable);
      if (result <= 0)
        continue;
      prepared = true;
    }
    size_t watches = 0;
    for (SWWatch *watch = app->watches; watch; watch = watch->next)
      if (!watch->removing)
        ++watches;
    size_t transfers = sw_transfer_count(app);
    size_t count = 3 + watches + transfers;
    struct pollfd *fds = calloc(count, sizeof(*fds));
    SWWatch **ready = watches ? calloc(watches, sizeof(*ready)) : NULL;
    if (!fds || (watches && !ready)) {
      if (prepared)
        sw_wayland_read(app, false);
      free(fds);
      free(ready);
      return sw_error(app, SWErrorNative, ENOMEM);
    }
    fds[0] = (struct pollfd){app->disconnected ? -1 : app->display_fd,
                             POLLIN | (writable ? POLLOUT : 0), 0};
    fds[1] = (struct pollfd){app->wake_fd, POLLIN, 0};
    int extra =
        app->wayland && !app->disconnected ? sw_wayland_extra_fd(app) : -1;
    fds[2] = (struct pollfd){extra == app->display_fd ? -1 : extra, POLLIN, 0};
    size_t index = 0;
    for (SWWatch *watch = app->watches; watch; watch = watch->next) {
      if (watch->removing)
        continue;
      ready[index] = watch;
      fds[3 + index++] = (struct pollfd){watch->fd, POLLIN, 0};
    }
    sw_transfer_poll(app, fds + 3 + watches);
    int result = poll(fds, count, timeout_ms(app));
    int error = errno;
    if (prepared)
      sw_wayland_read(app, result > 0 && (fds[0].revents & POLLIN));
    if (result < 0) {
      free(ready);
      free(fds);
      if (error == EINTR)
        continue;
      return sw_error(app, SWErrorNative, error);
    }
    if (fds[0].revents & (POLLERR | POLLHUP | POLLNVAL))
      sw_disconnect(app, EPIPE);
    if (fds[1].revents & POLLIN) {
      uint64_t value;
      while (read(app->wake_fd, &value, sizeof(value)) < 0 && errno == EINTR) {
      }
      app->pump = true;
    }
    sw_transfer_ready(app, fds + 3 + watches);
    for (index = 0; index < watches; ++index) {
      SWWatch *watch = ready[index];
      if (fds[3 + index].revents && !watch->removing)
        app->callback(app->context, watch->identity, 2);
    }
    free(ready);
    free(fds);
  }
  return 0;
}

void scoop_window_application_wake(void *pointer) { sw_wake(pointer); }
void scoop_window_application_stop(void *pointer) {
  ((SWApp *)pointer)->stopped = true;
}
void scoop_window_application_deadline(void *pointer, double seconds) {
  SWApp *app = pointer;
  int64_t now = sw_now();
  if (seconds < 0)
    app->deadline = -1;
  else if (seconds >= (double)(INT64_MAX - now) / 1000000000.0)
    app->deadline = INT64_MAX;
  else
    app->deadline = now + (int64_t)ceil(seconds * 1000000000.0);
  sw_wake(app);
}

void scoop_window_application_destroy(void *pointer) {
  SWApp *app = pointer;
  sw_transfer_destroy(app);
  if (app->wayland)
    sw_wayland_destroy(app);
  if (app->x11)
    sw_x11_destroy(app);
  sw_monitors_free(app->monitors);
  while (app->events) {
    SWEvent *event = app->events;
    app->events = event->next;
    sw_event_free(event);
  }
  close(app->wake_fd);
  free(app);
}

void *scoop_window_watch_create(void *pointer, int64_t identity, int32_t fd) {
  SWApp *app = pointer;
  if (fd < 0) {
    errno = EINVAL;
    return NULL;
  }
  SWWatch *watch = calloc(1, sizeof(*watch));
  if (!watch)
    return NULL;
  *watch = (SWWatch){app, identity, fd, false, app->watches};
  app->watches = watch;
  sw_wake(app);
  return watch;
}
void scoop_window_watch_remove(void *pointer) {
  SWWatch *watch = pointer;
  watch->removing = true;
  sw_wake(watch->app);
}
