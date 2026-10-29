#include "x11_peer.h"

static void preedit(XTServer *server, const char *text, uint32_t caret,
                    uint32_t first, uint32_t count) {
  size_t length = 0;
  char *encoded = xcb_utf8_to_compound_text(text, strlen(text), &length);
  if (!encoded || length > UINT16_MAX) {
    atomic_fetch_add(&server->errors, 1);
    free(encoded);
    return;
  }
  uint32_t feedback[7] = {0};
  xcb_im_preedit_draw_fr_t frame = {0};
  frame.caret = caret;
  frame.chg_first = first;
  frame.chg_length = count;
  frame.length_of_preedit_string = (uint16_t)length;
  frame.preedit_string = (uint8_t *)encoded;
  frame.feedback_array.size = strcmp(text, "中🙂") == 0 ? 2
                              : strcmp(text, "文") == 0 ? 1
                                                        : 7;
  frame.feedback_array.items = feedback;
  xcb_im_preedit_draw_callback(server->im, server->ic, &frame);
  free(encoded);
}
void xt_im_command(XTServer *server, int command) {
  if (!server->im || !server->ic) {
    server->pending_im = command;
    return;
  }
  server->pending_im = 0;
  switch (command) {
  case 4:
    xcb_im_preedit_start_callback(server->im, server->ic);
    preedit(server, "中🙂", 2, 0, 0);
    break;
  case 5:
    preedit(server, "文", 2, 0, 1);
    break;
  case 6: {
    xcb_im_preedit_done_callback(server->im, server->ic);
    size_t length = 0;
    const char *text = "完成🙂";
    char *encoded = xcb_utf8_to_compound_text(text, strlen(text), &length);
    if (encoded)
      xcb_im_commit_string(server->im, server->ic, XCB_XIM_LOOKUP_CHARS,
                           encoded, (uint32_t)length, 0);
    else
      atomic_fetch_add(&server->errors, 1);
    free(encoded);
    break;
  }
  case 7:
    preedit(server, "invalid", 0, 99, 1);
    break;
  default:
    atomic_fetch_add(&server->errors, 1);
    break;
  }
  xcb_flush(server->connection);
}
static void callback(xcb_im_t *im, xcb_im_client_t *client,
                     xcb_im_input_context_t *ic,
                     const xcb_im_packet_header_fr_t *header, void *frame,
                     void *arg, void *data) {
  XTServer *server = data;
  if (header->major_opcode == XCB_XIM_SET_IC_FOCUS) {
    server->ic = ic;
    if (server->pending_im)
      xt_im_command(server, server->pending_im);
  } else if (header->major_opcode == XCB_XIM_DESTROY_IC) {
    if (server->ic == ic)
      server->ic = NULL;
  } else if (header->major_opcode == XCB_XIM_FORWARD_EVENT) {
    xcb_im_forward_event(im, ic, arg);
  }
}
bool xt_im_start(XTServer *server) {
  xcb_compound_text_init();
  int screen_number;
  server->connection = xcb_connect(NULL, &screen_number);
  if (xcb_connection_has_error(server->connection))
    return false;
  xcb_screen_iterator_t screens =
      xcb_setup_roots_iterator(xcb_get_setup(server->connection));
  for (int i = 0; i < screen_number; ++i)
    xcb_screen_next(&screens);
  if (!screens.rem)
    return false;
  xcb_screen_t *screen = screens.data;
  xcb_window_t window = xcb_generate_id(server->connection);
  xcb_create_window(server->connection, XCB_COPY_FROM_PARENT, window,
                    screen->root, 0, 0, 1, 1, 0, XCB_WINDOW_CLASS_INPUT_OUTPUT,
                    screen->root_visual, 0, NULL);
  char name[64], modifiers[80];
  snprintf(name, sizeof(name), "scoop-test-%ld", (long)getpid());
  snprintf(modifiers, sizeof(modifiers), "@im=%s", name);
  const char *old = getenv("XMODIFIERS");
  if (old)
    server->old_modifiers = strdup(old);
  if (setenv("XMODIFIERS", modifiers, 1) != 0)
    return false;
  uint32_t supported[] = {XCB_IM_PreeditCallbacks | XCB_IM_StatusNothing};
  xcb_im_styles_t styles = {1, supported};
  char *encoding_names[] = {"COMPOUND_TEXT"};
  xcb_im_encodings_t encodings = {1, encoding_names};
  server->im = xcb_im_create(server->connection, screen_number, window, name,
                             XCB_IM_ALL_LOCALES, &styles, NULL, NULL,
                             &encodings, 0, callback, server);
  if (!server->im || !xcb_im_open_im(server->im))
    return false;
  xcb_im_set_use_sync_mode(server->im, false);
  xcb_im_set_use_sync_event(server->im, false);
  xcb_flush(server->connection);
  return true;
}
void xt_im_stop(XTServer *server) {
  if (server->im) {
    xcb_im_close_im(server->im);
    xcb_im_destroy(server->im);
  }
  if (server->connection) {
    xcb_disconnect(server->connection);
    if (server->old_modifiers)
      setenv("XMODIFIERS", server->old_modifiers, 1);
    else
      unsetenv("XMODIFIERS");
  }
  free(server->old_modifiers);
}
