#include "x11.h"
#include <X11/Xlib-xcb.h>
#include <wchar.h>
#include <xkbcommon/xkbcommon-keysyms.h>
#include <xkbcommon/xkbcommon-x11.h>

void sw_x11_keymap(SWX11 *x) {
  xcb_connection_t *connection = XGetXCBConnection(x->display);
  int32_t device = xkb_x11_get_core_keyboard_device_id(connection);
  struct xkb_keymap *keymap =
      device >= 0
          ? xkb_x11_keymap_new_from_device(x->keyboard.context, connection,
                                           device, XKB_KEYMAP_COMPILE_NO_FLAGS)
          : NULL;
  if (!keymap) {
    sw_error(x->app, SWErrorNative, EINVAL);
    x->app->error_pending = true;
    return;
  }
  struct xkb_state *state =
      xkb_x11_state_new_from_device(keymap, connection, device);
  if (!state) {
    xkb_keymap_unref(keymap);
    sw_error(x->app, SWErrorNative, ENOMEM);
    x->app->error_pending = true;
    return;
  }
  if (x->focused)
    sw_keyboard_reset(x->focused, &x->keyboard);
  xkb_state_unref(x->keyboard.state);
  xkb_keymap_unref(x->keyboard.keymap);
  x->keyboard.state = state;
  x->keyboard.keymap = keymap;
}

void sw_x11_text_begin(SWWindow *window) {
  if (!window->x->text_batch) {
    window->x->text_was_preedit = window->preedit_active;
    window->x->text_invalid = false;
    window->x->text_batch = sw_event(window, SWTextBatch);
  }
}

void sw_x11_text_end(SWWindow *window) {
  SWEvent *event = window->x->text_batch;
  if (window->x->text_invalid && event) {
    if (window->x->ic) {
      char *discard = Xutf8ResetIC(window->x->ic);
      if (discard)
        XFree(discard);
    }
    sw_event_free(event);
    event = NULL;
    if (window->x->text_was_preedit) {
      event = sw_event(window, SWTextBatch);
      sw_text_edit(event, 0, "", -1, -1);
    }
    window->preedit_active = false;
  }
  window->x->text_batch = NULL;
  if (!event)
    return;
  if (event->edit_count || event->data.flags == -1)
    sw_enqueue(window->app, event);
  else
    sw_event_free(event);
}

static void edit(SWWindow *window, int kind, const char *text, int64_t caret) {
  bool own_batch = !window->x->text_batch;
  sw_x11_text_begin(window);
  sw_text_edit(window->x->text_batch, kind, text, caret, caret);
  if (own_batch)
    sw_x11_text_end(window);
}

static void clear_preedit(SWWindow *window) {
  SWXWindow *native = window->x;
  if (window->preedit_active)
    edit(window, 0, "", -1);
  window->preedit_active = false;
  native->preedit_started = false;
  free(native->preedit);
  native->preedit = NULL;
  native->preedit_length = 0;
}

static void reject_preedit(SWWindow *window) {
  bool own_batch = !window->x->text_batch;
  sw_x11_text_begin(window);
  window->x->text_invalid = true;
  window->preedit_active = false;
  window->x->preedit_started = false;
  free(window->x->preedit);
  window->x->preedit = NULL;
  window->x->preedit_length = 0;
  if (own_batch)
    sw_x11_text_end(window);
}

static int preedit_start(XIC ic, XPointer data, XPointer call) {
  ((SWWindow *)data)->x->preedit_started = true;
  return -1;
}
static void preedit_done(XIC ic, XPointer data, XPointer call) {
  clear_preedit((SWWindow *)data);
}

static void publish_preedit(SWWindow *window) {
  SWXWindow *native = window->x;
  if (native->preedit_length > (SIZE_MAX - 1) / 7) {
    sw_error(window->app, SWErrorNative, EOVERFLOW);
    window->app->error_pending = true;
    return;
  }
  char *text = calloc(native->preedit_length * 7 + 1, 1);
  if (!text) {
    sw_error(window->app, SWErrorNative, ENOMEM);
    window->app->error_pending = true;
    return;
  }
  size_t bytes = 0, caret = 0;
  for (size_t i = 0; i < native->preedit_length; ++i) {
    if (i == (size_t)native->preedit_cursor)
      caret = bytes;
    int length = xkb_keysym_to_utf8(xkb_utf32_to_keysym(native->preedit[i]),
                                    text + bytes, 7);
    if (length <= 0) {
      free(text);
      reject_preedit(window);
      return;
    }
    bytes += (size_t)length - 1;
  }
  if ((size_t)native->preedit_cursor >= native->preedit_length)
    caret = bytes;
  edit(window, 0, text, (int64_t)caret);
  window->preedit_active = bytes != 0;
  free(text);
}

static void preedit_draw(XIC ic, XPointer data, XPointer call) {
  SWWindow *window = (SWWindow *)data;
  SWXWindow *native = window->x;
  XIMPreeditDrawCallbackStruct *draw = (XIMPreeditDrawCallbackStruct *)call;
  if (!draw || draw->chg_first < 0 || draw->chg_length < 0 ||
      (size_t)draw->chg_first > native->preedit_length ||
      (size_t)draw->chg_length >
          native->preedit_length - (size_t)draw->chg_first) {
    reject_preedit(window);
    return;
  }
  size_t added = draw->text ? draw->text->length : 0;
  size_t size = native->preedit_length - (size_t)draw->chg_length + added;
  uint32_t *text = calloc(size ? size : 1, sizeof(*text));
  if (!text) {
    sw_error(window->app, SWErrorNative, ENOMEM);
    window->app->error_pending = true;
    return;
  }
  size_t first = (size_t)draw->chg_first;
  if (first)
    memcpy(text, native->preedit, first * sizeof(*text));
  const char *bytes = added && !draw->text->encoding_is_wchar
                          ? draw->text->string.multi_byte
                          : NULL;
  size_t remaining = bytes ? strlen(bytes) : 0;
  mbstate_t state = {0};
  for (size_t i = 0; i < added; ++i) {
    wchar_t character;
    if (draw->text->encoding_is_wchar)
      character = draw->text->string.wide_char[i];
    else {
      size_t length = mbrtowc(&character, bytes, remaining, &state);
      if (length == (size_t)-1 || length == (size_t)-2 || length == 0) {
        free(text);
        reject_preedit(window);
        return;
      }
      bytes += length;
      remaining -= length;
    }
    text[first + i] = (uint32_t)character;
  }
  size_t tail = native->preedit_length - first - (size_t)draw->chg_length;
  if (tail)
    memcpy(text + first + added, native->preedit + first + draw->chg_length,
           tail * sizeof(*text));
  free(native->preedit);
  native->preedit = text;
  native->preedit_length = size;
  native->preedit_cursor = draw->caret;
  if (draw->caret < 0 || (size_t)draw->caret > size) {
    reject_preedit(window);
    return;
  }
  publish_preedit(window);
}

static void preedit_caret(XIC ic, XPointer data, XPointer call) {
  SWWindow *window = (SWWindow *)data;
  XIMPreeditCaretCallbackStruct *caret = (XIMPreeditCaretCallbackStruct *)call;
  if (!caret)
    return;
  int position = window->x->preedit_cursor;
  if (caret->direction == XIMAbsolutePosition)
    position = caret->position;
  else if (caret->direction == XIMForwardChar)
    ++position;
  else if (caret->direction == XIMBackwardChar)
    --position;
  if (position < 0)
    position = 0;
  if ((size_t)position > window->x->preedit_length)
    position = (int)window->x->preedit_length;
  caret->position = window->x->preedit_cursor = position;
  publish_preedit(window);
}

void sw_x11_input_init(SWWindow *window) {
  SWX11 *x = window->app->x11;
  if (!x->im || !x->im_style)
    return;
  XVaNestedList preedit = NULL;
  // Xlib stores every callback as XIMProc, including this documented int
  // result.
  XIMCallback start = {(XPointer)window,
                       (XIMProc)(void (*)(void))preedit_start};
  XIMCallback done = {(XPointer)window, (XIMProc)preedit_done};
  XIMCallback draw = {(XPointer)window, (XIMProc)preedit_draw};
  XIMCallback caret = {(XPointer)window, (XIMProc)preedit_caret};
  if (x->im_style & XIMPreeditCallbacks)
    preedit = XVaCreateNestedList(
        0, XNPreeditStartCallback, &start, XNPreeditDoneCallback, &done,
        XNPreeditDrawCallback, &draw, XNPreeditCaretCallback, &caret, NULL);
  else if (x->im_style & XIMPreeditPosition) {
    XPoint spot = {0, 0};
    preedit = XVaCreateNestedList(0, XNSpotLocation, &spot, NULL);
  }
  if (preedit)
    window->x->ic = XCreateIC(x->im, XNInputStyle, x->im_style, XNClientWindow,
                              window->x->id, XNFocusWindow, window->x->id,
                              XNPreeditAttributes, preedit, NULL);
  else
    window->x->ic =
        XCreateIC(x->im, XNInputStyle, x->im_style, XNClientWindow,
                  window->x->id, XNFocusWindow, window->x->id, NULL);
  if (preedit)
    XFree(preedit);
}

void sw_x11_input_destroy(SWWindow *window) {
  clear_preedit(window);
  if (window->x->text_batch) {
    sw_event_free(window->x->text_batch);
    window->x->text_batch = NULL;
  }
  if (window->x->ic) {
    XDestroyIC(window->x->ic);
    window->x->ic = NULL;
  }
}

void sw_x11_input_focus(SWWindow *window, bool focused) {
  SWX11 *x = window->app->x11;
  if (focused) {
    x->focused = window;
    if (window->ime_allowed && window->x->ic)
      XSetICFocus(window->x->ic);
  } else {
    if (window->x->ic)
      XUnsetICFocus(window->x->ic);
    clear_preedit(window);
    sw_keyboard_reset(window, &x->keyboard);
    if (x->focused == window)
      x->focused = NULL;
  }
}

int sw_x11_ime(SWWindow *window, bool allowed) {
  if (allowed && !window->x->ic)
    return SWUnsupported;
  if (window->ime_allowed == allowed)
    return SWApplied;
  if (!allowed) {
    clear_preedit(window);
    if (window->x->ic) {
      XUnsetICFocus(window->x->ic);
      char *discard = Xutf8ResetIC(window->x->ic);
      if (discard)
        XFree(discard);
    }
  }
  window->ime_allowed = allowed;
  sw_emit(window, SWImeState, allowed);
  if (allowed && window->snapshot.focused)
    XSetICFocus(window->x->ic);
  return SWApplied;
}

void sw_x11_caret(SWWindow *window) {
  SWX11 *x = window->app->x11;
  if (!window->x->ic || !(x->im_style & XIMPreeditPosition))
    return;
  double px = window->caret_x * window->snapshot.scale;
  double py = (window->caret_y + window->caret_height) * window->snapshot.scale;
  XPoint spot = {(short)fmax(SHRT_MIN, fmin(SHRT_MAX, px)),
                 (short)fmax(SHRT_MIN, fmin(SHRT_MAX, py))};
  XVaNestedList values = XVaCreateNestedList(0, XNSpotLocation, &spot, NULL);
  if (values) {
    XSetICValues(window->x->ic, XNPreeditAttributes, values, NULL);
    XFree(values);
  }
}

void sw_x11_key(SWWindow *window, XKeyEvent *event, bool repeat) {
  SWX11 *x = window->app->x11;
  bool pressed = event->type == KeyPress;
  if (x->keyboard.state && !repeat)
    xkb_state_update_key(x->keyboard.state, event->keycode,
                         pressed ? XKB_KEY_DOWN : XKB_KEY_UP);
  KeySym symbol = NoSymbol;
  char scratch[64];
  XLookupString(event, scratch, sizeof(scratch), &symbol, NULL);
  char logical[7] = {0};
  xkb_keysym_to_utf8((uint32_t)symbol, logical, sizeof(logical));
  if (pressed && window->ime_allowed && window->x->ic) {
    if (event->state & (ControlMask | Mod1Mask | Mod4Mask)) {
      sw_key(window, &x->keyboard, event->keycode, (uint32_t)symbol, logical,
             true, repeat, false);
      return;
    }
    bool composing = window->x->preedit_started || window->preedit_active;
    Status status;
    int length = Xutf8LookupString(window->x->ic, event, scratch,
                                   sizeof(scratch), &symbol, &status);
    char *text = NULL;
    if (status == XBufferOverflow) {
      if (length < 0 || length == INT_MAX) {
        sw_error(window->app, SWErrorNative, EOVERFLOW);
        window->app->error_pending = true;
        return;
      }
      text = calloc((size_t)length + 1, 1);
      if (!text) {
        sw_error(window->app, SWErrorNative, ENOMEM);
        window->app->error_pending = true;
        return;
      }
      int capacity = length + 1;
      length = Xutf8LookupString(window->x->ic, event, text, capacity, &symbol,
                                 &status);
      if (length < 0 || length >= capacity || status == XBufferOverflow) {
        free(text);
        reject_preedit(window);
        return;
      }
      text[length] = 0;
    } else {
      text = malloc((size_t)(length > 0 ? length : 0) + 1);
      if (!text) {
        sw_error(window->app, SWErrorNative, ENOMEM);
        window->app->error_pending = true;
        return;
      }
      if (length > 0)
        memcpy(text, scratch, (size_t)length);
      text[length > 0 ? length : 0] = 0;
    }
    if ((status == XLookupChars || status == XLookupBoth) && length > 0)
      edit(window, 1, text, -1);
    free(text);
    if (composing || window->x->preedit_started)
      return;
    sw_key(window, &x->keyboard, event->keycode, (uint32_t)symbol, logical,
           true, repeat, false);
  } else
    sw_key(window, &x->keyboard, event->keycode, (uint32_t)symbol, logical,
           pressed, repeat, !window->ime_allowed);
}
