#include "wayland.h"

struct SWTransfer {
  int fd;
  SWDrop *drop;
  SWOffer *offer;
  SWTransfer *next;
};

SWDrop *sw_drop_new(SWWindow *window) {
  SWDrop *drop = calloc(1, sizeof(*drop));
  if (!drop) {
    sw_error(window->app, SWErrorNative, ENOMEM);
    window->app->error_pending = true;
    return NULL;
  }
  drop->window = window;
  drop->generation = window->snapshot.generation;
  return drop;
}

bool sw_drop_append(SWDrop *drop, const void *bytes, size_t length) {
  if (length > SIZE_MAX - drop->length || drop->length + length > INT64_MAX) {
    sw_error(drop->window->app, SWErrorNative, EOVERFLOW);
    drop->window->app->error_pending = true;
    return false;
  }
  size_t needed = drop->length + length;
  if (needed > drop->capacity) {
    size_t capacity =
        drop->capacity > SIZE_MAX / 2 ? needed : drop->capacity * 2;
    if (capacity < needed)
      capacity = needed;
    uint8_t *data = realloc(drop->bytes, capacity);
    if (!data) {
      sw_error(drop->window->app, SWErrorNative, ENOMEM);
      drop->window->app->error_pending = true;
      return false;
    }
    drop->bytes = data;
    drop->capacity = capacity;
  }
  if (length)
    memcpy(drop->bytes + drop->length, bytes, length);
  drop->length = needed;
  return true;
}

static void drop_event(SWDrop *drop, int kind) {
  SWWindow *window = drop->window;
  if (window->snapshot.generation != drop->generation)
    return;
  SWEvent *event = sw_event(window, SWFileDrop);
  if (!event)
    return;
  if (kind != 2 && drop->length) {
    uint8_t *copy = malloc(drop->length);
    if (!copy) {
      sw_event_free(event);
      sw_error(window->app, SWErrorNative, ENOMEM);
      window->app->error_pending = true;
      return;
    }
    memcpy(copy, drop->bytes, drop->length);
    event->data.text = copy;
    event->data.text_length = (int64_t)drop->length;
  }
  event->data.flags = kind;
  sw_enqueue(window->app, event);
}

void sw_drop_complete(SWDrop *drop) {
  drop->loaded = true;
  if (!drop->entered) {
    drop_event(drop, 0);
    drop->entered = true;
  }
  if (drop->dropped)
    drop_event(drop, 1);
}
void sw_drop_accept(SWDrop *drop) {
  if (drop->dropped)
    return;
  drop->dropped = true;
  if (drop->loaded)
    drop_event(drop, 1);
}
void sw_drop_leave(SWDrop *drop) {
  if (drop->entered && !drop->dropped)
    drop_event(drop, 2);
}
void sw_drop_free(SWDrop *drop) {
  if (!drop)
    return;
  free(drop->bytes);
  free(drop);
}

bool sw_transfer_start(SWDrop *drop, SWOffer *offer, int fd) {
  SWTransfer *transfer = calloc(1, sizeof(*transfer));
  if (!transfer) {
    sw_error(drop->window->app, SWErrorNative, ENOMEM);
    drop->window->app->error_pending = true;
    close(fd);
    return false;
  }
  SWApp *app = drop->window->app;
  *transfer = (SWTransfer){fd, drop, offer, app->transfers};
  app->transfers = transfer;
  return true;
}
size_t sw_transfer_count(SWApp *app) {
  size_t count = 0;
  for (SWTransfer *item = app->transfers; item; item = item->next)
    ++count;
  return count;
}
void sw_transfer_poll(SWApp *app, struct pollfd *fds) {
  for (SWTransfer *item = app->transfers; item; item = item->next)
    *fds++ = (struct pollfd){item->fd, POLLIN, 0};
}
void sw_transfer_ready(SWApp *app, struct pollfd *fds) {
  SWTransfer **link = &app->transfers;
  while (*link) {
    SWTransfer *item = *link;
    short ready = (fds++)->revents;
    if (!ready) {
      link = &item->next;
      continue;
    }
    bool finished = false, success = true;
    // One pipe chunk per turn leaves display input and owner controls
    // serviceable.
    uint8_t bytes[16384];
    ssize_t count;
    do {
      count = read(item->fd, bytes, sizeof(bytes));
    } while (count < 0 && errno == EINTR);
    if (count > 0) {
      if (!sw_drop_append(item->drop, bytes, (size_t)count)) {
        success = false;
        finished = true;
      }
    } else if (!count)
      finished = true;
    else if (errno != EAGAIN && errno != EWOULDBLOCK) {
      success = false;
      finished = true;
    }
    if (!finished) {
      link = &item->next;
      continue;
    }
    *link = item->next;
    close(item->fd);
    SWOffer *offer = item->offer;
    free(item);
    sw_wayland_drop_received(offer, success);
  }
}
void sw_transfer_cancel(SWApp *app, SWDrop *drop) {
  SWTransfer **link = &app->transfers;
  while (*link) {
    SWTransfer *item = *link;
    if (item->drop != drop) {
      link = &item->next;
      continue;
    }
    *link = item->next;
    close(item->fd);
    free(item);
  }
}
void sw_transfer_destroy(SWApp *app) {
  while (app->transfers) {
    SWTransfer *item = app->transfers;
    app->transfers = item->next;
    close(item->fd);
    free(item);
  }
}
