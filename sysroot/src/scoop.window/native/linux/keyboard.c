#include "window.h"
#include <xkbcommon/xkbcommon-keysyms.h>

bool sw_keyboard_init(SWKeyboardState *keyboard) {
  keyboard->context = xkb_context_new(XKB_CONTEXT_NO_FLAGS);
  if (!keyboard->context)
    return false;
  const char *locale = getenv("LC_ALL");
  if (!locale || !*locale)
    locale = getenv("LC_CTYPE");
  if (!locale || !*locale)
    locale = getenv("LANG");
  if (!locale || !*locale)
    locale = "C";
  keyboard->compose_table = xkb_compose_table_new_from_locale(
      keyboard->context, locale, XKB_COMPOSE_COMPILE_NO_FLAGS);
  if (keyboard->compose_table)
    keyboard->compose = xkb_compose_state_new(keyboard->compose_table,
                                              XKB_COMPOSE_STATE_NO_FLAGS);
  return true;
}

void sw_keyboard_destroy(SWKeyboardState *keyboard) {
  while (keyboard->pressed) {
    SWPressedKey *next = keyboard->pressed->next;
    free(keyboard->pressed->text);
    free(keyboard->pressed);
    keyboard->pressed = next;
  }
  xkb_compose_state_unref(keyboard->compose);
  xkb_compose_table_unref(keyboard->compose_table);
  xkb_state_unref(keyboard->state);
  xkb_keymap_unref(keyboard->keymap);
  xkb_context_unref(keyboard->context);
  memset(keyboard, 0, sizeof(*keyboard));
}

uint64_t sw_keyboard_modifiers(SWKeyboardState *keyboard) {
  if (!keyboard->state)
    return 0;
  const char *names[] = {XKB_MOD_NAME_SHIFT, XKB_MOD_NAME_CTRL,
                         XKB_MOD_NAME_ALT,   XKB_MOD_NAME_LOGO,
                         XKB_MOD_NAME_CAPS,  XKB_MOD_NAME_NUM,
                         "ScrollLock"};
  uint64_t result = 0;
  for (size_t i = 0; i < sizeof(names) / sizeof(names[0]); ++i)
    if (xkb_state_mod_name_is_active(keyboard->state, names[i],
                                     XKB_STATE_MODS_EFFECTIVE) > 0)
      result |= UINT64_C(1) << i;
  const uint32_t sides[] = {50, 62, 37, 105, 64, 108, 133, 134};
  for (SWPressedKey *key = keyboard->pressed; key; key = key->next)
    for (size_t i = 0; i < sizeof(sides) / sizeof(sides[0]); ++i)
      if (key->code == sides[i])
        result |= UINT64_C(1) << (16 + i);
  return result;
}

static void modifiers_changed(SWWindow *window, SWKeyboardState *keyboard) {
  uint64_t value = sw_keyboard_modifiers(keyboard);
  if (value == keyboard->modifiers)
    return;
  keyboard->modifiers = value;
  SWEvent *event = sw_event(window, SWModifiers);
  if (event) {
    event->data.a = (int64_t)value;
    sw_enqueue(window->app, event);
  }
}

static void emit_key(SWWindow *window, SWKeyboardState *keyboard,
                     SWPressedKey *key, int flags) {
  SWEvent *event = sw_event(window, SWKeyboard);
  if (!event)
    return;
  event->data.a = key->code;
  event->data.b = (int64_t)keyboard->modifiers;
  event->data.c = key->symbol;
  event->data.flags = flags;
  if (key->symbol >= XKB_KEY_dead_grave && key->symbol <= XKB_KEY_dead_greek)
    event->data.flags |= 8;
  if (key->text) {
    event->data.text = (const uint8_t *)strdup(key->text);
    if (event->data.text)
      event->data.text_length = (int64_t)strlen(key->text);
  }
  sw_enqueue(window->app, event);
}

static void compose(SWWindow *window, SWKeyboardState *keyboard,
                    uint32_t symbol, const char *text) {
  if (keyboard->modifiers & (2 | 4 | 8))
    return;
  if (keyboard->compose && xkb_compose_state_feed(keyboard->compose, symbol) ==
                               XKB_COMPOSE_FEED_ACCEPTED) {
    switch (xkb_compose_state_get_status(keyboard->compose)) {
    case XKB_COMPOSE_COMPOSING:
      return;
    case XKB_COMPOSE_CANCELLED:
      xkb_compose_state_reset(keyboard->compose);
      return;
    case XKB_COMPOSE_COMPOSED: {
      int length = xkb_compose_state_get_utf8(keyboard->compose, NULL, 0);
      char *value = malloc((size_t)length + 1);
      if (!value) {
        sw_error(window->app, SWErrorNative, ENOMEM);
        window->app->error_pending = true;
        return;
      }
      xkb_compose_state_get_utf8(keyboard->compose, value, (size_t)length + 1);
      sw_commit(window, value);
      free(value);
      xkb_compose_state_reset(keyboard->compose);
      return;
    }
    case XKB_COMPOSE_NOTHING:
      break;
    }
  }
  if (text && (unsigned char)text[0] >= 32 && (unsigned char)text[0] != 127)
    sw_commit(window, text);
}

void sw_key(SWWindow *window, SWKeyboardState *keyboard, uint32_t code,
            uint32_t symbol, const char *text, bool pressed, bool repeat,
            bool commit) {
  SWPressedKey **link = &keyboard->pressed;
  while (*link && (*link)->code != code)
    link = &(*link)->next;
  SWPressedKey *key = *link;
  if (!pressed && !key)
    return;
  if (pressed && !key) {
    key = calloc(1, sizeof(*key));
    if (!key) {
      sw_error(window->app, SWErrorNative, ENOMEM);
      window->app->error_pending = true;
      return;
    }
    key->code = code;
    key->symbol = symbol;
    key->text = text ? strdup(text) : NULL;
    *link = key;
  }
  if (!pressed)
    *link = key->next;
  modifiers_changed(window, keyboard);
  emit_key(window, keyboard, key, (pressed ? 1 : 0) | (repeat ? 2 : 0));
  if (pressed && commit)
    compose(window, keyboard, symbol, text);
  if (!pressed) {
    free(key->text);
    free(key);
  }
}

void sw_keyboard_key(SWWindow *window, SWKeyboardState *keyboard, uint32_t code,
                     bool pressed, bool repeat, bool commit) {
  if (!keyboard->state)
    return;
  uint32_t symbol = xkb_state_key_get_one_sym(keyboard->state, code);
  int length = xkb_state_key_get_utf8(keyboard->state, code, NULL, 0);
  char *text = calloc((size_t)length + 1, 1);
  if (!text) {
    sw_error(window->app, SWErrorNative, ENOMEM);
    window->app->error_pending = true;
    return;
  }
  if (length)
    xkb_state_key_get_utf8(keyboard->state, code, text, (size_t)length + 1);
  sw_key(window, keyboard, code, symbol, text, pressed, repeat, commit);
  free(text);
}

void sw_keyboard_reset(SWWindow *window, SWKeyboardState *keyboard) {
  sw_cancel_preedit(window);
  if (keyboard->compose)
    xkb_compose_state_reset(keyboard->compose);
  while (keyboard->pressed) {
    SWPressedKey *key = keyboard->pressed;
    keyboard->pressed = key->next;
    emit_key(window, keyboard, key, 4);
    free(key->text);
    free(key);
  }
  if (keyboard->state)
    xkb_state_update_mask(
        keyboard->state, 0, 0,
        xkb_state_serialize_mods(keyboard->state, XKB_STATE_MODS_LOCKED), 0, 0,
        xkb_state_serialize_layout(keyboard->state, XKB_STATE_LAYOUT_LOCKED));
  modifiers_changed(window, keyboard);
}
