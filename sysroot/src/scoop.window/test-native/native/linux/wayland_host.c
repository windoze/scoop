#include "wayland_host.h"
#include <errno.h>
#include <signal.h>
#include <sys/mman.h>
#include <xkbcommon/xkbcommon.h>

void wt_destroy(struct wl_client *client, struct wl_resource *resource) {
  wl_resource_destroy(resource);
}
static void surface_gone(struct wl_resource *resource) {
  WTSurface *surface = wl_resource_get_user_data(resource);
  if (surface->server->window == surface)
    surface->server->window = NULL;
  atomic_fetch_sub(&surface->server->surfaces, 1);
  free(surface);
}
static void surface_attach(struct wl_client *client,
                           struct wl_resource *resource,
                           struct wl_resource *buffer, int32_t x, int32_t y) {
  ((WTSurface *)wl_resource_get_user_data(resource))->buffer = buffer;
}
static void surface_damage(struct wl_client *client,
                           struct wl_resource *resource, int32_t x, int32_t y,
                           int32_t width, int32_t height) {
  // Damage is irrelevant to this peer; real pixels are validated by the
  // renderer tests.
}
static void surface_frame(struct wl_client *client,
                          struct wl_resource *resource, uint32_t id) {
  WTSurface *surface = wl_resource_get_user_data(resource);
  surface->frame = wl_resource_create(client, &wl_callback_interface, 1, id);
}
static void surface_region(struct wl_client *client,
                           struct wl_resource *resource,
                           struct wl_resource *region) {
  // Regions are accepted; this peer has no compositor hit testing or occlusion.
}
static void surface_transform(struct wl_client *client,
                              struct wl_resource *resource, int32_t value) {
  if (value != WL_OUTPUT_TRANSFORM_NORMAL)
    wl_resource_post_error(resource, WL_SURFACE_ERROR_INVALID_TRANSFORM,
                           "unexpected transform");
}
static void surface_scale(struct wl_client *client,
                          struct wl_resource *resource, int32_t value) {
  WTSurface *surface = wl_resource_get_user_data(resource);
  surface->buffer_scale = value;
  if (surface->server->window == surface)
    atomic_store(&surface->server->buffer_scale, value);
}
void wt_configure(WTSurface *surface, int width, int height) {
  if (!surface || !surface->top)
    return;
  struct wl_array states;
  wl_array_init(&states);
  if (surface->maximized) {
    uint32_t *state = wl_array_add(&states, sizeof(*state));
    *state = XDG_TOPLEVEL_STATE_MAXIMIZED;
  }
  xdg_toplevel_send_configure(surface->top, width, height, &states);
  wl_array_release(&states);
  xdg_surface_send_configure(surface->xdg,
                             wl_display_next_serial(surface->server->display));
}
static void surface_commit(struct wl_client *client,
                           struct wl_resource *resource) {
  WTSurface *surface = wl_resource_get_user_data(resource);
  WTServer *server = surface->server;
  if (surface->top && !surface->configured) {
    surface->configured = true;
    if (server->output)
      wl_surface_send_enter(resource, server->output);
    if (surface->decoration)
      zxdg_toplevel_decoration_v1_send_configure(
          surface->decoration, ZXDG_TOPLEVEL_DECORATION_V1_MODE_SERVER_SIDE);
    struct wl_array caps;
    wl_array_init(&caps);
    if (server->features) {
      uint32_t *values = wl_array_add(&caps, 2 * sizeof(*values));
      values[0] = XDG_TOPLEVEL_WM_CAPABILITIES_MAXIMIZE;
      values[1] = XDG_TOPLEVEL_WM_CAPABILITIES_MINIMIZE;
    }
    xdg_toplevel_send_wm_capabilities(surface->top, &caps);
    wl_array_release(&caps);
    wt_configure(surface, 0, 0);
    wt_enter(server);
  }
  if (surface->buffer) {
    wl_buffer_send_release(surface->buffer);
    surface->buffer = NULL;
  }
  if (surface->frame) {
    wl_callback_send_done(surface->frame, 1);
    wl_resource_destroy(surface->frame);
    surface->frame = NULL;
  }
  wt_execute(server);
}
static const struct wl_surface_interface surface_impl = {
    .destroy = wt_destroy,
    .attach = surface_attach,
    .damage = surface_damage,
    .frame = surface_frame,
    .set_opaque_region = surface_region,
    .set_input_region = surface_region,
    .commit = surface_commit,
    .set_buffer_transform = surface_transform,
    .set_buffer_scale = surface_scale,
    .damage_buffer = surface_damage};
static void create_surface(struct wl_client *client,
                           struct wl_resource *resource, uint32_t id) {
  WTServer *server = wl_resource_get_user_data(resource);
  WTSurface *surface = calloc(1, sizeof(*surface));
  if (!surface) {
    wl_client_post_no_memory(client);
    return;
  }
  surface->server = server;
  surface->buffer_scale = 1;
  surface->surface = wl_resource_create(client, &wl_surface_interface, 4, id);
  if (!surface->surface) {
    free(surface);
    wl_client_post_no_memory(client);
    return;
  }
  atomic_fetch_add(&server->surfaces, 1);
  wl_resource_set_implementation(surface->surface, &surface_impl, surface,
                                 surface_gone);
}
static void region_edit(struct wl_client *client, struct wl_resource *resource,
                        int32_t x, int32_t y, int32_t width, int32_t height) {
  // No region-based policy is exercised by this protocol peer.
}
static const struct wl_region_interface region_impl = {
    .destroy = wt_destroy, .add = region_edit, .subtract = region_edit};
static void create_region(struct wl_client *client,
                          struct wl_resource *resource, uint32_t id) {
  struct wl_resource *region =
      wl_resource_create(client, &wl_region_interface, 1, id);
  wl_resource_set_implementation(region, &region_impl, NULL, NULL);
}
static const struct wl_compositor_interface compositor_impl = {
    .create_surface = create_surface, .create_region = create_region};
static void bind_compositor(struct wl_client *client, void *data,
                            uint32_t version, uint32_t id) {
  WTServer *server = data;
  server->client = client;
  struct wl_resource *resource =
      wl_resource_create(client, &wl_compositor_interface, 4, id);
  wl_resource_set_implementation(resource, &compositor_impl, server, NULL);
}
static void top_destroy(struct wl_client *client,
                        struct wl_resource *resource) {
  WTSurface *surface = wl_resource_get_user_data(resource);
  surface->top = NULL;
  surface->configured = false;
  wl_resource_destroy(resource);
}
static void top_text(struct wl_client *client, struct wl_resource *resource,
                     const char *text) {
  // Application identities do not affect the exercised protocol responses.
}
static void top_title(struct wl_client *client, struct wl_resource *resource,
                      const char *text) {
  // This marker follows all queued titles after the paused reader resumes.
  if (!strcmp(text, "scoop-backpressure-end"))
    wt_configure(wl_resource_get_user_data(resource), 311, 191);
}
static void top_limits(struct wl_client *client, struct wl_resource *resource,
                       int32_t w, int32_t h) {
  if (w < 0 || h < 0)
    wl_resource_post_error(resource, XDG_TOPLEVEL_ERROR_INVALID_SIZE,
                           "negative size");
}
static void top_maximize(struct wl_client *client,
                         struct wl_resource *resource) {
  WTSurface *surface = wl_resource_get_user_data(resource);
  surface->maximized = true;
  if (surface->configured)
    wt_configure(surface, 700, 500);
}
static void top_unmaximize(struct wl_client *client,
                           struct wl_resource *resource) {
  WTSurface *surface = wl_resource_get_user_data(resource);
  surface->maximized = false;
  if (surface->configured)
    wt_configure(surface, 0, 0);
}
static void top_minimize(struct wl_client *client,
                         struct wl_resource *resource) {
  // xdg-shell has no observed minimized state to send back.
}
static const struct xdg_toplevel_interface top_impl = {
    .destroy = top_destroy,
    .set_title = top_title,
    .set_app_id = top_text,
    .set_max_size = top_limits,
    .set_min_size = top_limits,
    .set_maximized = top_maximize,
    .unset_maximized = top_unmaximize,
    .set_minimized = top_minimize};
static void get_toplevel(struct wl_client *client, struct wl_resource *resource,
                         uint32_t id) {
  WTSurface *surface = wl_resource_get_user_data(resource);
  surface->top = wl_resource_create(client, &xdg_toplevel_interface, 6, id);
  wl_resource_set_implementation(surface->top, &top_impl, surface, NULL);
}
static void xdg_destroy(struct wl_client *client,
                        struct wl_resource *resource) {
  ((WTSurface *)wl_resource_get_user_data(resource))->xdg = NULL;
  wl_resource_destroy(resource);
}
static void xdg_geometry(struct wl_client *client, struct wl_resource *resource,
                         int32_t x, int32_t y, int32_t w, int32_t h) {
  if (w <= 0 || h <= 0)
    wl_resource_post_error(resource, XDG_SURFACE_ERROR_INVALID_SIZE,
                           "invalid geometry");
}
static void xdg_ack(struct wl_client *client, struct wl_resource *resource,
                    uint32_t serial) {
  // Each configure has a fresh serial; tests observe the resulting complete
  // metrics.
}
static const struct xdg_surface_interface xdg_impl = {
    .destroy = xdg_destroy,
    .get_toplevel = get_toplevel,
    .set_window_geometry = xdg_geometry,
    .ack_configure = xdg_ack};
static void get_xdg(struct wl_client *client, struct wl_resource *resource,
                    uint32_t id, struct wl_resource *native) {
  WTSurface *surface = wl_resource_get_user_data(native);
  surface->server->window = surface;
  surface->xdg = wl_resource_create(client, &xdg_surface_interface, 6, id);
  wl_resource_set_implementation(surface->xdg, &xdg_impl, surface, NULL);
}
static void wm_pong(struct wl_client *client, struct wl_resource *resource,
                    uint32_t serial) {
  // The test server does not use ping timeouts.
}
static const struct xdg_wm_base_interface wm_impl = {
    .destroy = wt_destroy, .get_xdg_surface = get_xdg, .pong = wm_pong};
static void bind_wm(struct wl_client *client, void *data, uint32_t version,
                    uint32_t id) {
  struct wl_resource *resource =
      wl_resource_create(client, &xdg_wm_base_interface, 6, id);
  wl_resource_set_implementation(resource, &wm_impl, data, NULL);
}
static const struct wl_output_interface output_impl = {.release = wt_destroy};
static void bind_output(struct wl_client *client, void *data, uint32_t version,
                        uint32_t id) {
  WTServer *server = data;
  server->output = wl_resource_create(client, &wl_output_interface, 4, id);
  wl_resource_set_implementation(server->output, &output_impl, server, NULL);
  wl_output_send_geometry(server->output, 0, 0, 300, 200,
                          WL_OUTPUT_SUBPIXEL_UNKNOWN, "Scoop", "protocol test",
                          WL_OUTPUT_TRANSFORM_NORMAL);
  wl_output_send_mode(server->output, WL_OUTPUT_MODE_CURRENT, 1600, 900, 60000);
  wl_output_send_scale(server->output, 2);
  wl_output_send_name(server->output, "SCOOP-TEST-1");
  wl_output_send_done(server->output);
  if (server->window) {
    wl_surface_send_enter(server->window->surface, server->output);
    wt_configure(server->window, 317, 197);
  }
}
void wt_output_change(WTServer *server, int command) {
  if (command == 13) {
    struct wl_array caps = {0};
    xdg_toplevel_send_wm_capabilities(server->window->top, &caps);
    wl_output_send_geometry(server->output, 0, 0, 300, 200,
                            WL_OUTPUT_SUBPIXEL_UNKNOWN, "Scoop",
                            "protocol test", WL_OUTPUT_TRANSFORM_90);
    wl_output_send_scale(server->output, 3);
    wl_output_send_done(server->output);
    wp_fractional_scale_v1_send_preferred_scale(server->window->fractional,
                                                180);
    wt_configure(server->window, 313, 193);
  } else if (command == 14) {
    wl_surface_send_leave(server->window->surface, server->output);
    wl_global_destroy(server->output_global);
    server->output_global = NULL;
    server->output = NULL;
    wt_configure(server->window, 315, 195);
  } else {
    server->output_global = wl_global_create(
        server->display, &wl_output_interface, 4, server, bind_output);
  }
}
static void decor_destroy(struct wl_client *client,
                          struct wl_resource *resource) {
  ((WTSurface *)wl_resource_get_user_data(resource))->decoration = NULL;
  wl_resource_destroy(resource);
}
static void decor_mode(struct wl_client *client, struct wl_resource *resource,
                       uint32_t mode) {
  if (mode != ZXDG_TOPLEVEL_DECORATION_V1_MODE_SERVER_SIDE)
    zxdg_toplevel_decoration_v1_send_configure(resource, mode);
}
static void decor_unset(struct wl_client *client,
                        struct wl_resource *resource) {
  zxdg_toplevel_decoration_v1_send_configure(
      resource, ZXDG_TOPLEVEL_DECORATION_V1_MODE_SERVER_SIDE);
}
static const struct zxdg_toplevel_decoration_v1_interface decor_impl = {
    .destroy = decor_destroy,
    .set_mode = decor_mode,
    .unset_mode = decor_unset};
static void get_decor(struct wl_client *client, struct wl_resource *resource,
                      uint32_t id, struct wl_resource *top) {
  WTSurface *surface = wl_resource_get_user_data(top);
  surface->decoration =
      wl_resource_create(client, &zxdg_toplevel_decoration_v1_interface, 1, id);
  wl_resource_set_implementation(surface->decoration, &decor_impl, surface,
                                 NULL);
}
static const struct zxdg_decoration_manager_v1_interface decor_manager_impl = {
    .destroy = wt_destroy, .get_toplevel_decoration = get_decor};
static void bind_decor(struct wl_client *client, void *data, uint32_t version,
                       uint32_t id) {
  struct wl_resource *resource =
      wl_resource_create(client, &zxdg_decoration_manager_v1_interface, 1, id);
  wl_resource_set_implementation(resource, &decor_manager_impl, data, NULL);
}
static void viewport_source(struct wl_client *client,
                            struct wl_resource *resource, wl_fixed_t x,
                            wl_fixed_t y, wl_fixed_t width, wl_fixed_t height) {
  // The backend uses the complete buffer and does not crop it.
}
static void viewport_destination(struct wl_client *client,
                                 struct wl_resource *resource, int32_t width,
                                 int32_t height) {
  WTSurface *surface = wl_resource_get_user_data(resource);
  atomic_store(&surface->server->destination_width, width);
  atomic_store(&surface->server->destination_height, height);
}
static const struct wp_viewport_interface viewport_impl = {
    .destroy = wt_destroy,
    .set_source = viewport_source,
    .set_destination = viewport_destination};
static void get_viewport(struct wl_client *client, struct wl_resource *resource,
                         uint32_t id, struct wl_resource *native) {
  WTSurface *surface = wl_resource_get_user_data(native);
  surface->viewport = wl_resource_create(client, &wp_viewport_interface, 1, id);
  wl_resource_set_implementation(surface->viewport, &viewport_impl, surface,
                                 NULL);
}
static const struct wp_viewporter_interface viewporter_impl = {
    .destroy = wt_destroy, .get_viewport = get_viewport};
static void bind_viewporter(struct wl_client *client, void *data,
                            uint32_t version, uint32_t id) {
  struct wl_resource *resource =
      wl_resource_create(client, &wp_viewporter_interface, 1, id);
  wl_resource_set_implementation(resource, &viewporter_impl, data, NULL);
}
static const struct wp_fractional_scale_v1_interface fractional_impl = {
    .destroy = wt_destroy};
static void get_fractional(struct wl_client *client,
                           struct wl_resource *resource, uint32_t id,
                           struct wl_resource *native) {
  WTSurface *surface = wl_resource_get_user_data(native);
  surface->fractional =
      wl_resource_create(client, &wp_fractional_scale_v1_interface, 1, id);
  wl_resource_set_implementation(surface->fractional, &fractional_impl, surface,
                                 NULL);
  wp_fractional_scale_v1_send_preferred_scale(surface->fractional, 150);
}
static const struct wp_fractional_scale_manager_v1_interface
    fractional_manager_impl = {.destroy = wt_destroy,
                               .get_fractional_scale = get_fractional};
static void bind_fractional(struct wl_client *client, void *data,
                            uint32_t version, uint32_t id) {
  struct wl_resource *resource = wl_resource_create(
      client, &wp_fractional_scale_manager_v1_interface, 1, id);
  wl_resource_set_implementation(resource, &fractional_manager_impl, data,
                                 NULL);
}
static int commands_ready(int fd, uint32_t mask, void *data) {
  uint64_t value;
  while (read(fd, &value, sizeof(value)) > 0) {
  }
  wt_execute(data);
  return 0;
}
static void *server_thread(void *data) {
  WTServer *server = data;
  sigset_t signals;
  sigemptyset(&signals);
  sigaddset(&signals, SIGPIPE);
  pthread_sigmask(SIG_BLOCK, &signals, NULL);
  wl_display_run(server->display);
  wl_display_destroy_clients(server->display);
  return NULL;
}
void *scoop_test_wayland_start(int32_t features) {
  WTServer *server = calloc(1, sizeof(*server));
  if (!server)
    return NULL;
  server->wake = -1;
  server->features = features;
  server->display = wl_display_create();
  if (!server->display)
    goto failed;
  const char *name = wl_display_add_socket_auto(server->display);
  if (!name)
    goto failed;
  server->name = strdup(name);
  if (!server->name)
    goto failed;
  if (wl_display_init_shm(server->display) != 0)
    goto failed;
  wl_global_create(server->display, &wl_compositor_interface, 4, server,
                   bind_compositor);
  wl_global_create(server->display, &xdg_wm_base_interface, 6, server, bind_wm);
  server->output_global = wl_global_create(
      server->display, &wl_output_interface, 4, server, bind_output);
  wl_global_create(server->display, &zxdg_decoration_manager_v1_interface, 1,
                   server, bind_decor);
  wl_global_create(server->display, &wl_seat_interface, 8, server,
                   wt_bind_seat);
  wl_global_create(server->display, &wl_data_device_manager_interface, 3,
                   server, wt_bind_data);
  if (features) {
    wl_global_create(server->display, &wp_viewporter_interface, 1, server,
                     bind_viewporter);
    wl_global_create(server->display, &wp_fractional_scale_manager_v1_interface,
                     1, server, bind_fractional);
    wl_global_create(server->display, &zwp_text_input_manager_v3_interface, 1,
                     server, wt_bind_text);
  }
  struct xkb_context *context = xkb_context_new(XKB_CONTEXT_NO_FLAGS);
  struct xkb_rule_names rules = {.layout = "us", .variant = "intl"};
  struct xkb_keymap *map =
      context ? xkb_keymap_new_from_names(context, &rules,
                                          XKB_KEYMAP_COMPILE_NO_FLAGS)
              : NULL;
  server->keymap =
      map ? xkb_keymap_get_as_string(map, XKB_KEYMAP_FORMAT_TEXT_V1) : NULL;
  xkb_keymap_unref(map);
  xkb_context_unref(context);
  if (!server->keymap)
    goto failed;
  server->keymap_size = (uint32_t)strlen(server->keymap) + 1;
  server->wake = eventfd(0, EFD_CLOEXEC | EFD_NONBLOCK);
  if (server->wake < 0)
    goto failed;
  server->commands = wl_event_loop_add_fd(
      wl_display_get_event_loop(server->display), server->wake,
      WL_EVENT_READABLE, commands_ready, server);
  if (!server->commands ||
      pthread_create(&server->thread, NULL, server_thread, server))
    goto failed;
  return server;
failed:
  if (server->commands)
    wl_event_source_remove(server->commands);
  if (server->wake >= 0)
    close(server->wake);
  if (server->display)
    wl_display_destroy(server->display);
  free(server->name);
  free(server->keymap);
  free(server);
  return NULL;
}
const char *scoop_test_wayland_name(void *pointer) {
  return ((WTServer *)pointer)->name;
}
void scoop_test_wayland_command(void *pointer, int32_t command,
                                int32_t minimum_serial) {
  WTServer *server = pointer;
  atomic_store(&server->minimum_serial, minimum_serial);
  atomic_store(&server->command, command);
  uint64_t value = 1;
  if (write(server->wake, &value, sizeof(value)) != sizeof(value))
    atomic_fetch_add(&server->failures, 1);
}
int32_t scoop_test_wayland_stat(void *pointer, int32_t index) {
  WTServer *server = pointer;
  switch (index) {
  case 0:
    return atomic_load(&server->last_text_serial);
  case 1:
    return atomic_load(&server->destination_width);
  case 2:
    return atomic_load(&server->destination_height);
  case 3:
    return atomic_load(&server->buffer_scale);
  case 4:
    return atomic_load(&server->finished_drops);
  case 5:
    return atomic_load(&server->failures);
  default:
    return -1;
  }
}
void scoop_test_wayland_stop(void *pointer) {
  WTServer *server = pointer;
  scoop_test_wayland_command(server, -1, 0);
  pthread_join(server->thread, NULL);
  wl_event_source_remove(server->commands);
  close(server->wake);
  wl_display_destroy(server->display);
  free(server->name);
  free(server->keymap);
  free(server->surrounding);
  free(server);
}
