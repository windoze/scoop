#include "wayland.h"
#include <fcntl.h>

struct SWOffer {
  SWSeat *seat;
  struct wl_data_offer *native;
  bool uri_list;
  uint32_t source_actions, action;
  SWDrop *drop;
  SWOffer *next;
};

static void offer_destroy(SWOffer *offer) {
  SWSeat *seat = offer->seat;
  if (seat->drag_offer == offer) {
    seat->drag_offer = NULL;
    seat->drag_window = NULL;
  }
  SWOffer **link = &seat->offers;
  while (*link != offer)
    link = &(*link)->next;
  *link = offer->next;
  if (offer->drop)
    sw_transfer_cancel(seat->wayland->app, offer->drop);
  sw_drop_free(offer->drop);
  wl_data_offer_destroy(offer->native);
  free(offer);
}
static void offer_finish(SWOffer *offer) {
  if (wl_data_offer_get_version(offer->native) >= 3)
    wl_data_offer_finish(offer->native);
  offer_destroy(offer);
}
static void offer_type(void *data, struct wl_data_offer *native,
                       const char *mime) {
  if (!strcmp(mime, "text/uri-list"))
    ((SWOffer *)data)->uri_list = true;
}
static void offer_source_actions(void *data, struct wl_data_offer *native,
                                 uint32_t actions) {
  ((SWOffer *)data)->source_actions = actions;
}
static void offer_action(void *data, struct wl_data_offer *native,
                         uint32_t action) {
  ((SWOffer *)data)->action = action;
}
static const struct wl_data_offer_listener offer_listener = {
    .offer = offer_type,
    .source_actions = offer_source_actions,
    .action = offer_action};
static SWOffer *find_offer(SWSeat *seat, struct wl_data_offer *native) {
  for (SWOffer *offer = seat->offers; offer; offer = offer->next)
    if (offer->native == native)
      return offer;
  return NULL;
}
static void data_offer(void *data, struct wl_data_device *device,
                       struct wl_data_offer *native) {
  SWSeat *seat = data;
  SWOffer *offer = calloc(1, sizeof(*offer));
  if (!offer) {
    wl_data_offer_destroy(native);
    sw_error(seat->wayland->app, SWErrorNative, ENOMEM);
    seat->wayland->app->error_pending = true;
    return;
  }
  offer->seat = seat;
  offer->native = native;
  offer->next = seat->offers;
  seat->offers = offer;
  wl_data_offer_add_listener(native, &offer_listener, offer);
}
static void data_enter(void *data, struct wl_data_device *device,
                       uint32_t serial, struct wl_surface *surface,
                       wl_fixed_t x, wl_fixed_t y,
                       struct wl_data_offer *native) {
  SWSeat *seat = data;
  SWOffer *offer = find_offer(seat, native);
  SWWindow *window = sw_wayland_find(seat->wayland, surface);
  if (!offer)
    return;
  seat->drag_offer = offer;
  seat->drag_window = window;
  bool copy = wl_data_offer_get_version(native) < 3 ||
              (offer->source_actions & WL_DATA_DEVICE_MANAGER_DND_ACTION_COPY);
  if (!window || !offer->uri_list || !copy) {
    wl_data_offer_accept(native, serial, NULL);
    return;
  }
  offer->drop = sw_drop_new(window);
  if (!offer->drop)
    return;
  wl_data_offer_accept(native, serial, "text/uri-list");
  if (wl_data_offer_get_version(native) >= 3)
    wl_data_offer_set_actions(native, WL_DATA_DEVICE_MANAGER_DND_ACTION_COPY,
                              WL_DATA_DEVICE_MANAGER_DND_ACTION_COPY);
  int fds[2];
  if (pipe2(fds, O_CLOEXEC | O_NONBLOCK) < 0) {
    sw_error(window->app, SWErrorNative, errno);
    window->app->error_pending = true;
    offer_destroy(offer);
    return;
  }
  // The writer is handed to the source; only our read end must be nonblocking.
  int flags = fcntl(fds[1], F_GETFL);
  if (flags < 0 || fcntl(fds[1], F_SETFL, flags & ~O_NONBLOCK) < 0) {
    int error = errno;
    close(fds[0]);
    close(fds[1]);
    offer_destroy(offer);
    sw_error(window->app, SWErrorNative, error);
    window->app->error_pending = true;
    return;
  }
  wl_data_offer_receive(native, "text/uri-list", fds[1]);
  close(fds[1]);
  if (!sw_transfer_start(offer->drop, offer, fds[0]))
    offer_destroy(offer);
}
static void data_leave(void *data, struct wl_data_device *device) {
  SWSeat *seat = data;
  SWOffer *offer = seat->drag_offer;
  seat->drag_offer = NULL;
  seat->drag_window = NULL;
  if (!offer || (offer->drop && offer->drop->dropped))
    return;
  if (offer->drop)
    sw_drop_leave(offer->drop);
  offer_destroy(offer);
}
static void data_motion(void *data, struct wl_data_device *device,
                        uint32_t time, wl_fixed_t x, wl_fixed_t y) {
  // File payload events have no motion coordinates; pointer input owns
  // position.
  (void)data;
  (void)device;
  (void)time;
  (void)x;
  (void)y;
}
static void data_drop(void *data, struct wl_data_device *device) {
  SWSeat *seat = data;
  SWOffer *offer = seat->drag_offer;
  if (!offer)
    return;
  if (!offer->drop ||
      (wl_data_offer_get_version(offer->native) >= 3 &&
       offer->action != WL_DATA_DEVICE_MANAGER_DND_ACTION_COPY)) {
    if (offer->drop)
      sw_drop_leave(offer->drop);
    offer_destroy(offer);
    return;
  }
  sw_drop_accept(offer->drop);
  if (offer->drop->loaded)
    offer_finish(offer);
}
static void data_selection(void *data, struct wl_data_device *device,
                           struct wl_data_offer *native) {
  SWSeat *seat = data;
  SWOffer *offer = find_offer(seat, native);
  if (offer && offer != seat->drag_offer)
    offer_destroy(offer);
}
static const struct wl_data_device_listener device_listener = {
    .data_offer = data_offer,
    .enter = data_enter,
    .leave = data_leave,
    .motion = data_motion,
    .drop = data_drop,
    .selection = data_selection};

void sw_wayland_drop_received(SWOffer *offer, bool success) {
  if (!success) {
    sw_drop_leave(offer->drop);
    offer_destroy(offer);
    return;
  }
  sw_drop_complete(offer->drop);
  if (offer->drop->dropped)
    offer_finish(offer);
}
void sw_wayland_drop_setup(SWSeat *seat) {
  if (!seat->wayland->data_manager || seat->data_device)
    return;
  seat->data_device = wl_data_device_manager_get_data_device(
      seat->wayland->data_manager, seat->seat);
  wl_data_device_add_listener(seat->data_device, &device_listener, seat);
}
void sw_wayland_drop_destroy(SWSeat *seat) {
  while (seat->offers)
    offer_destroy(seat->offers);
  if (seat->data_device) {
    if (wl_data_device_get_version(seat->data_device) >= 2)
      wl_data_device_release(seat->data_device);
    else
      wl_data_device_destroy(seat->data_device);
  }
  seat->data_device = NULL;
}
void sw_wayland_drop_window_destroy(SWWindow *window) {
  for (SWSeat *seat = window->app->wayland->seats; seat; seat = seat->next) {
    SWOffer *offer = seat->offers;
    while (offer) {
      SWOffer *next = offer->next;
      if (offer->drop && offer->drop->window == window) {
        sw_drop_leave(offer->drop);
        offer_destroy(offer);
      }
      offer = next;
    }
  }
}
