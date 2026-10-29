#ifndef SCOOP_WINDOW_WAYLAND_H
#define SCOOP_WINDOW_WAYLAND_H

#include "protocols/fractional-scale-v1.h"
#include "protocols/text-input-unstable-v3.h"
#include "protocols/viewporter.h"
#include "protocols/xdg-activation-v1.h"
#include "protocols/xdg-decoration-unstable-v1.h"
#include "protocols/xdg-output-unstable-v1.h"
#include "protocols/xdg-shell.h"
#include "window.h"
#include <libdecor-0/libdecor.h>
#include <wayland-client.h>
#include <wayland-cursor.h>

typedef struct SWSeat SWSeat;
typedef struct SWOutput SWOutput;
typedef struct SWOutputRef SWOutputRef;
typedef struct SWOffer SWOffer;

struct SWWayland {
  SWApp *app;
  struct wl_display *display;
  struct wl_registry *registry;
  struct wl_compositor *compositor;
  struct wl_shm *shm;
  struct xdg_wm_base *wm;
  struct wp_viewporter *viewporter;
  struct wp_fractional_scale_manager_v1 *fractional;
  struct zxdg_output_manager_v1 *output_manager;
  struct zxdg_decoration_manager_v1 *decorations;
  struct zwp_text_input_manager_v3 *text_manager;
  struct wl_data_device_manager *data_manager;
  struct xdg_activation_v1 *activation;
  struct libdecor *decor;
  SWOutput *outputs;
  SWSeat *seats;
};

struct SWOutput {
  SWWayland *wayland;
  uint32_t global;
  struct wl_output *output;
  struct zxdg_output_v1 *description;
  SWMonitor *monitor;
  int scale, logical_width, logical_height, logical_x, logical_y;
  int pixel_width, pixel_height, transform;
  bool logical_size, logical_position;
  SWOutput *next;
};
struct SWOutputRef {
  SWOutput *output;
  SWOutputRef *next;
};

struct SWWLWindow {
  struct wl_surface *surface;
  struct xdg_surface *xdg_surface;
  struct xdg_toplevel *toplevel;
  struct zxdg_toplevel_decoration_v1 *decoration;
  struct libdecor_frame *frame;
  struct wp_viewport *viewport;
  struct wp_fractional_scale_v1 *fractional;
  struct wl_callback *frame_callback;
  struct xdg_activation_token_v1 *activation_token;
  SWOutputRef *outputs;
  int pending_width, pending_height;
  uint32_t pending_states;
  bool use_decor, need_decor;
  bool requested_maximized;
  bool capabilities_known, can_maximize, can_minimize;
  double fractional_scale;
};

typedef struct SWTextState {
  uint32_t serial;
  int64_t revision;
  char *text;
  int32_t anchor, cursor;
  struct SWTextState *next;
} SWTextState;

struct SWSeat {
  SWWayland *wayland;
  uint32_t global;
  struct wl_seat *seat;
  struct wl_pointer *pointer;
  struct wl_keyboard *keyboard;
  struct wl_surface *cursor_surface;
  struct wl_cursor_theme *cursor_theme;
  int cursor_scale;
  uint32_t pointer_serial, input_serial;
  SWWindow *pointer_window, *keyboard_window, *text_window;
  double pointer_x, pointer_y;
  double axis[2], steps[2];
  bool has_axis[2], has_steps[2], stopped[2], scrolling;
  uint32_t axis_source;
  SWKeyboardState keys;
  int repeat_rate, repeat_delay;
  uint32_t repeat_key;
  int64_t repeat_deadline;
  struct zwp_text_input_v3 *text_input;
  uint32_t text_serial;
  char *pending_preedit, *pending_commit;
  int32_t pending_begin, pending_end;
  uint32_t delete_before, delete_after;
  bool has_preedit, has_commit, has_delete, text_enabled;
  bool text_invalid, text_deferred, text_dirty;
  SWTextState *text_states, **text_states_tail;
  struct wl_data_device *data_device;
  SWOffer *offers, *drag_offer;
  SWWindow *drag_window;
  SWSeat *next;
};

void sw_wayland_output_description(SWOutput *output);
void sw_wayland_scale(SWWindow *window);
int sw_wayland_dimension(int configured, double requested, double previous,
                         double fallback, double minimum, double maximum);
void sw_wayland_configured(SWWindow *window, int width, int height,
                           bool maximized);
int sw_wayland_visibility(SWWindow *window, bool visible);
void sw_wayland_limits(SWWindow *window);
void sw_wayland_buttons(SWWindow *window);
void sw_wayland_seat(SWWayland *wayland, uint32_t name, uint32_t version);
void sw_wayland_seat_destroy(SWSeat *seat);
void sw_wayland_cursor(SWSeat *seat);
void sw_wayland_input_destroy(SWWindow *window);
SWWindow *sw_wayland_find(SWWayland *wayland, struct wl_surface *surface);
void sw_wayland_text_setup(SWSeat *seat);
void sw_wayland_text_update(SWSeat *seat);
void sw_wayland_text_destroy(SWSeat *seat);
void sw_wayland_text_disable(SWSeat *seat);
int sw_wayland_ime(SWWindow *window, bool allowed);
void sw_wayland_drop_setup(SWSeat *seat);
void sw_wayland_drop_destroy(SWSeat *seat);
void sw_wayland_drop_window_destroy(SWWindow *window);
void sw_wayland_drop_received(SWOffer *offer, bool success);
bool sw_transfer_start(SWDrop *drop, SWOffer *offer, int fd);

#endif
