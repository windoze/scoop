#include "wayland_host.h"
#include <errno.h>
#include <fcntl.h>
#include <linux/input-event-codes.h>
#include <poll.h>
#include <sys/mman.h>

static void pointer_cursor(struct wl_client *client,
                           struct wl_resource *resource, uint32_t serial,
                           struct wl_resource *surface, int32_t x, int32_t y) {
  // The protocol peer accepts cursor surfaces; Weston verifies their
  // presentation.
}
static const struct wl_pointer_interface pointer_impl = {
    .set_cursor = pointer_cursor, .release = wt_destroy};
static const struct wl_keyboard_interface keyboard_impl = {.release =
                                                               wt_destroy};
static void get_pointer(struct wl_client *client, struct wl_resource *resource,
                        uint32_t id) {
  WTServer *server = wl_resource_get_user_data(resource);
  server->pointer = wl_resource_create(client, &wl_pointer_interface, 8, id);
  wl_resource_set_implementation(server->pointer, &pointer_impl, server, NULL);
}
static void get_keyboard(struct wl_client *client, struct wl_resource *resource,
                         uint32_t id) {
  WTServer *server = wl_resource_get_user_data(resource);
  server->keyboard = wl_resource_create(client, &wl_keyboard_interface, 8, id);
  wl_resource_set_implementation(server->keyboard, &keyboard_impl, server,
                                 NULL);
  int fd = memfd_create("scoop-test-keymap", MFD_CLOEXEC);
  if (fd < 0 ||
      write(fd, server->keymap, server->keymap_size) != server->keymap_size) {
    if (fd >= 0)
      close(fd);
    wl_client_post_no_memory(client);
    return;
  }
  wl_keyboard_send_keymap(server->keyboard, WL_KEYBOARD_KEYMAP_FORMAT_XKB_V1,
                          fd, server->keymap_size);
  close(fd);
  wl_keyboard_send_repeat_info(server->keyboard, 40, 30);
}
static const struct wl_seat_interface seat_impl = {.get_pointer = get_pointer,
                                                   .get_keyboard = get_keyboard,
                                                   .release = wt_destroy};
void wt_bind_seat(struct wl_client *client, void *data, uint32_t version,
                  uint32_t id) {
  WTServer *server = data;
  server->seat = wl_resource_create(client, &wl_seat_interface, 8, id);
  wl_resource_set_implementation(server->seat, &seat_impl, server, NULL);
  wl_seat_send_name(server->seat, "Scoop protocol seat");
  wl_seat_send_capabilities(server->seat, WL_SEAT_CAPABILITY_POINTER |
                                              WL_SEAT_CAPABILITY_KEYBOARD);
}
void wt_enter(WTServer *server) {
  if (!server->window)
    return;
  struct wl_resource *surface = server->window->surface;
  if (server->keyboard) {
    struct wl_array keys;
    wl_array_init(&keys);
    wl_keyboard_send_enter(server->keyboard,
                           wl_display_next_serial(server->display), surface,
                           &keys);
    wl_array_release(&keys);
    wl_keyboard_send_modifiers(
        server->keyboard, wl_display_next_serial(server->display), 0, 0, 0, 0);
  }
  if (server->pointer) {
    wl_pointer_send_enter(
        server->pointer, wl_display_next_serial(server->display), surface,
        wl_fixed_from_double(12.5), wl_fixed_from_double(20.25));
    wl_pointer_send_frame(server->pointer);
  }
  if (server->text)
    zwp_text_input_v3_send_enter(server->text, surface);
}
static void text_enable(struct wl_client *client,
                        struct wl_resource *resource) {
  ((WTServer *)wl_resource_get_user_data(resource))->enabled = true;
}
static void text_disable(struct wl_client *client,
                         struct wl_resource *resource) {
  ((WTServer *)wl_resource_get_user_data(resource))->enabled = false;
}
static void text_surrounding(struct wl_client *client,
                             struct wl_resource *resource, const char *text,
                             int32_t cursor, int32_t anchor) {
  WTServer *server = wl_resource_get_user_data(resource);
  free(server->surrounding);
  server->surrounding = strdup(text);
  server->cursor = cursor;
  server->anchor = anchor;
}
static void text_cause(struct wl_client *client, struct wl_resource *resource,
                       uint32_t value) {
  // All test edits are programmatic publications; cause does not change the
  // response.
}
static void text_content(struct wl_client *client, struct wl_resource *resource,
                         uint32_t hints, uint32_t purpose) {
  // The shared API currently uses the ordinary text content type.
}
static void text_rectangle(struct wl_client *client,
                           struct wl_resource *resource, int32_t x, int32_t y,
                           int32_t width, int32_t height) {
  if (width < 0 || height < 0)
    atomic_fetch_add(
        &((WTServer *)wl_resource_get_user_data(resource))->failures, 1);
}
static void text_commit(struct wl_client *client,
                        struct wl_resource *resource) {
  WTServer *server = wl_resource_get_user_data(resource);
  server->text_serial++;
  atomic_store(&server->last_text_serial, (int)server->text_serial);
  wt_execute(server);
}
static const struct zwp_text_input_v3_interface text_impl = {
    .destroy = wt_destroy,
    .enable = text_enable,
    .disable = text_disable,
    .set_surrounding_text = text_surrounding,
    .set_text_change_cause = text_cause,
    .set_content_type = text_content,
    .set_cursor_rectangle = text_rectangle,
    .commit = text_commit};
static void get_text(struct wl_client *client, struct wl_resource *resource,
                     uint32_t id, struct wl_resource *seat) {
  WTServer *server = wl_resource_get_user_data(resource);
  server->text =
      wl_resource_create(client, &zwp_text_input_v3_interface, 1, id);
  wl_resource_set_implementation(server->text, &text_impl, server, NULL);
}
static const struct zwp_text_input_manager_v3_interface text_manager_impl = {
    .destroy = wt_destroy, .get_text_input = get_text};
void wt_bind_text(struct wl_client *client, void *data, uint32_t version,
                  uint32_t id) {
  struct wl_resource *resource =
      wl_resource_create(client, &zwp_text_input_manager_v3_interface, 1, id);
  wl_resource_set_implementation(resource, &text_manager_impl, data, NULL);
}
static void offer_accept(struct wl_client *client, struct wl_resource *resource,
                         uint32_t serial, const char *mime) {
  if (mime && strcmp(mime, "text/uri-list"))
    atomic_fetch_add(
        &((WTServer *)wl_resource_get_user_data(resource))->failures, 1);
}
static void offer_receive(struct wl_client *client,
                          struct wl_resource *resource, const char *mime,
                          int fd) {
  WTServer *server = wl_resource_get_user_data(resource);
  const char text[] = "# native file URI "
                      "payload\r\nfile:///tmp/a%20b\r\nfile://localhost/tmp/"
                      "%FF\r\nhttps://example.com/no\r\n";
  if (strcmp(mime, "text/uri-list") ||
      write(fd, text, sizeof(text) - 1) != sizeof(text) - 1)
    atomic_fetch_add(&server->failures, 1);
  close(fd);
}
static void offer_finish(struct wl_client *client,
                         struct wl_resource *resource) {
  atomic_fetch_add(
      &((WTServer *)wl_resource_get_user_data(resource))->finished_drops, 1);
}
static void offer_actions(struct wl_client *client,
                          struct wl_resource *resource, uint32_t actions,
                          uint32_t preferred) {
  wl_data_offer_send_action(resource,
                            actions & WL_DATA_DEVICE_MANAGER_DND_ACTION_COPY);
  WTServer *server = wl_resource_get_user_data(resource);
  wl_data_device_send_drop(server->data_device);
}
static const struct wl_data_offer_interface offer_impl = {
    .accept = offer_accept,
    .receive = offer_receive,
    .destroy = wt_destroy,
    .finish = offer_finish,
    .set_actions = offer_actions};
static void data_selection(struct wl_client *client,
                           struct wl_resource *resource,
                           struct wl_resource *source, uint32_t serial) {
  // Clipboard selection is outside these drag-and-drop tests.
}
static const struct wl_data_device_interface data_impl = {
    .set_selection = data_selection, .release = wt_destroy};
static void get_data(struct wl_client *client, struct wl_resource *resource,
                     uint32_t id, struct wl_resource *seat) {
  WTServer *server = wl_resource_get_user_data(resource);
  server->data_device =
      wl_resource_create(client, &wl_data_device_interface, 3, id);
  wl_resource_set_implementation(server->data_device, &data_impl, server, NULL);
}
static const struct wl_data_device_manager_interface data_manager_impl = {
    .get_data_device = get_data};
void wt_bind_data(struct wl_client *client, void *data, uint32_t version,
                  uint32_t id) {
  struct wl_resource *resource =
      wl_resource_create(client, &wl_data_device_manager_interface, 3, id);
  wl_resource_set_implementation(resource, &data_manager_impl, data, NULL);
}
static void key(WTServer *server, uint32_t code, bool pressed) {
  wl_keyboard_send_key(
      server->keyboard, wl_display_next_serial(server->display), 1, code,
      pressed ? WL_KEYBOARD_KEY_STATE_PRESSED : WL_KEYBOARD_KEY_STATE_RELEASED);
}
static void tap(WTServer *server, uint32_t code) {
  key(server, code, true);
  key(server, code, false);
}
static void modifiers(WTServer *server, uint32_t mask) {
  wl_keyboard_send_modifiers(
      server->keyboard, wl_display_next_serial(server->display), mask, 0, 0, 0);
}
void wt_execute(WTServer *server) {
  int command = atomic_load(&server->command);
  if (!command)
    return;
  if (command == -1) {
    wl_display_terminate(server->display);
    return;
  }
  if (!server->window || !server->window->configured)
    return;
  if ((uint32_t)atomic_load(&server->minimum_serial) > server->text_serial)
    return;
  if (((command >= 2 && command <= 4) || command == 12) && !server->enabled)
    return;
  atomic_store(&server->command, 0);
  switch (command) {
  case 1:
    tap(server, KEY_A);
    key(server, KEY_LEFTSHIFT, true);
    modifiers(server, 1);
    tap(server, KEY_A);
    key(server, KEY_LEFTSHIFT, false);
    modifiers(server, 0);
    key(server, KEY_LEFTCTRL, true);
    modifiers(server, 4);
    tap(server, KEY_C);
    key(server, KEY_LEFTCTRL, false);
    modifiers(server, 0);
    tap(server, KEY_APOSTROPHE);
    tap(server, KEY_E);
    wl_pointer_send_motion(server->pointer, 2, wl_fixed_from_double(18.5),
                           wl_fixed_from_double(30.25));
    wl_pointer_send_button(server->pointer,
                           wl_display_next_serial(server->display), 2, BTN_LEFT,
                           WL_POINTER_BUTTON_STATE_PRESSED);
    wl_pointer_send_button(server->pointer,
                           wl_display_next_serial(server->display), 2, BTN_LEFT,
                           WL_POINTER_BUTTON_STATE_RELEASED);
    wl_pointer_send_axis_source(server->pointer, WL_POINTER_AXIS_SOURCE_WHEEL);
    wl_pointer_send_axis_value120(server->pointer,
                                  WL_POINTER_AXIS_VERTICAL_SCROLL, 120);
    wl_pointer_send_axis(server->pointer, 3, WL_POINTER_AXIS_VERTICAL_SCROLL,
                         wl_fixed_from_int(15));
    wl_pointer_send_frame(server->pointer);
    key(server, KEY_A, true);
    wl_keyboard_send_leave(server->keyboard,
                           wl_display_next_serial(server->display),
                           server->window->surface);
    wt_configure(server->window, 303, 183);
    break;
  case 2:
    zwp_text_input_v3_send_preedit_string(server->text, "中🙂", 7, 3);
    zwp_text_input_v3_send_done(server->text, server->text_serial);
    break;
  case 3:
    zwp_text_input_v3_send_delete_surrounding_text(server->text, 1, 1);
    zwp_text_input_v3_send_commit_string(server->text, "é");
    zwp_text_input_v3_send_preedit_string(server->text, "🙂", 4, 0);
    zwp_text_input_v3_send_done(server->text, server->text_serial);
    break;
  case 4:
    // One byte would split the preceding two-byte scalar in the published "é".
    zwp_text_input_v3_send_delete_surrounding_text(server->text, 1, 0);
    zwp_text_input_v3_send_commit_string(server->text, "must not commit");
    zwp_text_input_v3_send_done(server->text, server->text_serial);
    break;
  case 5: {
    struct wl_resource *offer =
        wl_resource_create(server->client, &wl_data_offer_interface, 3, 0);
    wl_resource_set_implementation(offer, &offer_impl, server, NULL);
    wl_data_device_send_data_offer(server->data_device, offer);
    wl_data_offer_send_offer(offer, "text/uri-list");
    wl_data_offer_send_source_actions(offer,
                                      WL_DATA_DEVICE_MANAGER_DND_ACTION_COPY);
    wl_data_device_send_enter(server->data_device,
                              wl_display_next_serial(server->display),
                              server->window->surface, 0, 0, offer);
    break;
  }
  case 6:
    wl_client_destroy(server->client);
    server->client = NULL;
    break;
  case 7:
    // This event follows all previously handled client requests and is a wire
    // barrier.
    wt_configure(server->window, 305, 185);
    break;
  case 8:
    key(server, KEY_A, true);
    break;
  case 9:
    key(server, KEY_A, false);
    wt_configure(server->window, 307, 187);
    break;
  case 10: {
    // Stop reading this client while it fills its outgoing socket buffer.
    wt_configure(server->window, 309, 189);
    wl_display_flush_clients(server->display);
    for (;;) {
      struct pollfd fd = {server->wake, POLLIN, 0};
      int status;
      do {
        status = poll(&fd, 1, -1);
      } while (status < 0 && errno == EINTR);
      if (status < 0) {
        atomic_fetch_add(&server->failures, 1);
        break;
      }
      uint64_t count;
      while (read(server->wake, &count, sizeof(count)) > 0) {
      }
      int next = atomic_exchange(&server->command, 0);
      if (next == -1) {
        wl_display_terminate(server->display);
        break;
      }
      if (next == 11) {
        break;
      }
      atomic_fetch_add(&server->failures, 1);
    }
    break;
  }
  case 12:
    zwp_text_input_v3_send_commit_string(server->text, "old");
    zwp_text_input_v3_send_done(server->text, server->text_serial - 1);
    zwp_text_input_v3_send_done(server->text, server->text_serial);
    break;
  case 13:
  case 14:
  case 15:
    wt_output_change(server, command);
    break;
  default:
    atomic_fetch_add(&server->failures, 1);
    break;
  }
  wl_display_flush_clients(server->display);
}
