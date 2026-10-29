#include "wayland.h"

static bool boundary(const char *text, size_t length, int64_t offset) {
  return offset >= 0 && (uint64_t)offset <= length &&
         ((size_t)offset == length ||
          ((unsigned char)text[offset] & 0xc0) != 0x80);
}

static bool utf8(const char *text) {
  const unsigned char *p = (const unsigned char *)text;
  while (*p) {
    uint32_t code = *p++;
    if (code < 0x80)
      continue;
    int count;
    uint32_t minimum;
    if (code >= 0xc2 && code <= 0xdf) {
      count = 1;
      minimum = 0x80;
      code &= 0x1f;
    } else if (code >= 0xe0 && code <= 0xef) {
      count = 2;
      minimum = 0x800;
      code &= 0x0f;
    } else if (code >= 0xf0 && code <= 0xf4) {
      count = 3;
      minimum = 0x10000;
      code &= 7;
    } else
      return false;
    while (count--) {
      if ((*p & 0xc0) != 0x80)
        return false;
      code = (code << 6) | (*p++ & 0x3f);
    }
    if (code < minimum || code > 0x10ffff || (code >= 0xd800 && code <= 0xdfff))
      return false;
  }
  return true;
}

static void pending_reset(SWSeat *seat) {
  free(seat->pending_preedit);
  free(seat->pending_commit);
  seat->pending_preedit = seat->pending_commit = NULL;
  seat->has_preedit = seat->has_commit = seat->has_delete = seat->text_invalid =
      false;
  seat->delete_before = seat->delete_after = 0;
}

static void state_free(SWTextState *state) {
  free(state->text);
  free(state);
}
static void states_reset(SWSeat *seat) {
  while (seat->text_states) {
    SWTextState *state = seat->text_states;
    seat->text_states = state->next;
    state_free(state);
  }
  seat->text_states_tail = &seat->text_states;
}

static void memory_failure(SWSeat *seat) {
  sw_error(seat->wayland->app, SWErrorNative, ENOMEM);
  seat->wayland->app->error_pending = true;
}

void sw_wayland_text_update(SWSeat *seat) {
  SWWindow *window = seat->text_window;
  if (!seat->text_input || !window || !window->ime_allowed)
    return;
  if (seat->text_deferred) {
    seat->text_dirty = true;
    return;
  }
  SWTextState *state = calloc(1, sizeof(*state));
  if (!state) {
    memory_failure(seat);
    return;
  }
  state->revision = -1;
  if (window->surrounding) {
    state->text = strdup(window->surrounding);
    if (!state->text) {
      free(state);
      memory_failure(seat);
      return;
    }
    state->revision = window->text_revision;
    state->anchor = window->selection_anchor;
    state->cursor = window->selection_cursor;
  }
  if (!seat->text_enabled) {
    zwp_text_input_v3_enable(seat->text_input);
    seat->text_enabled = true;
    seat->repeat_deadline = -1;
    if (seat->keys.compose)
      xkb_compose_state_reset(seat->keys.compose);
    zwp_text_input_v3_set_content_type(
        seat->text_input, ZWP_TEXT_INPUT_V3_CONTENT_HINT_NONE,
        ZWP_TEXT_INPUT_V3_CONTENT_PURPOSE_NORMAL);
  }
  if (state->text)
    zwp_text_input_v3_set_surrounding_text(seat->text_input, state->text,
                                           state->cursor, state->anchor);
  zwp_text_input_v3_set_text_change_cause(seat->text_input,
                                          ZWP_TEXT_INPUT_V3_CHANGE_CAUSE_OTHER);
  zwp_text_input_v3_set_cursor_rectangle(
      seat->text_input, (int32_t)floor(window->caret_x),
      (int32_t)floor(window->caret_y), (int32_t)ceil(window->caret_width),
      (int32_t)ceil(window->caret_height));
  zwp_text_input_v3_commit(seat->text_input);
  state->serial = ++seat->text_serial;
  *seat->text_states_tail = state;
  seat->text_states_tail = &state->next;
  seat->text_dirty = false;
}

void sw_wayland_text_disable(SWSeat *seat) {
  if (seat->text_enabled) {
    zwp_text_input_v3_disable(seat->text_input);
    zwp_text_input_v3_commit(seat->text_input);
    ++seat->text_serial;
  }
  seat->text_enabled = seat->text_deferred = seat->text_dirty = false;
  if (seat->text_window)
    sw_cancel_preedit(seat->text_window);
  pending_reset(seat);
  states_reset(seat);
}

static void text_enter(void *data, struct zwp_text_input_v3 *input,
                       struct wl_surface *surface) {
  SWSeat *seat = data;
  if (seat->text_window)
    sw_wayland_text_disable(seat);
  seat->text_window = sw_wayland_find(seat->wayland, surface);
  sw_wayland_text_update(seat);
}
static void text_leave(void *data, struct zwp_text_input_v3 *input,
                       struct wl_surface *surface) {
  SWSeat *seat = data;
  sw_wayland_text_disable(seat);
  seat->text_window = NULL;
}

static void text_preedit(void *data, struct zwp_text_input_v3 *input,
                         const char *text, int32_t begin, int32_t end) {
  SWSeat *seat = data;
  free(seat->pending_preedit);
  seat->pending_preedit = strdup(text ? text : "");
  seat->pending_begin = begin;
  seat->pending_end = end;
  seat->has_preedit = true;
  if (!seat->pending_preedit) {
    seat->text_invalid = true;
    memory_failure(seat);
  }
}
static void text_commit(void *data, struct zwp_text_input_v3 *input,
                        const char *text) {
  SWSeat *seat = data;
  free(seat->pending_commit);
  seat->pending_commit = strdup(text ? text : "");
  seat->has_commit = true;
  if (!seat->pending_commit) {
    seat->text_invalid = true;
    memory_failure(seat);
  }
}
static void text_delete(void *data, struct zwp_text_input_v3 *input,
                        uint32_t before, uint32_t after) {
  SWSeat *seat = data;
  seat->delete_before = before;
  seat->delete_after = after;
  seat->has_delete = true;
}

static bool valid_batch(SWSeat *seat, SWTextState *state) {
  if (seat->text_invalid)
    return false;
  if (seat->has_preedit) {
    const char *text = seat->pending_preedit;
    if (!utf8(text))
      return false;
    bool hidden = seat->pending_begin == -1 && seat->pending_end == -1;
    if (!hidden && (!boundary(text, strlen(text), seat->pending_begin) ||
                    !boundary(text, strlen(text), seat->pending_end)))
      return false;
  }
  if (seat->has_commit && !utf8(seat->pending_commit))
    return false;
  if (seat->has_delete && (seat->delete_before || seat->delete_after)) {
    if (!state || !state->text)
      return false;
    int64_t start =
        state->anchor < state->cursor ? state->anchor : state->cursor;
    int64_t end = state->anchor > state->cursor ? state->anchor : state->cursor;
    size_t length = strlen(state->text);
    if (!boundary(state->text, length, start - seat->delete_before) ||
        !boundary(state->text, length, end + seat->delete_after))
      return false;
  }
  return true;
}

static void text_done(void *data, struct zwp_text_input_v3 *input,
                      uint32_t serial) {
  SWSeat *seat = data;
  SWWindow *window = seat->text_window;
  if (!window || !seat->text_enabled) {
    pending_reset(seat);
    return;
  }
  SWTextState *state = seat->text_states;
  while (state && state->serial != serial)
    state = state->next;
  if (state) {
    while (seat->text_states != state) {
      SWTextState *old = seat->text_states;
      seat->text_states = old->next;
      state_free(old);
    }
  }
  if (!valid_batch(seat, state)) {
    // Reject the whole native transaction before publishing any of its edits.
    sw_wayland_text_disable(seat);
    sw_wayland_text_update(seat);
    return;
  }
  bool changed =
      window->preedit_active || seat->has_commit || seat->has_preedit ||
      (seat->has_delete && (seat->delete_before || seat->delete_after));
  if (changed) {
    SWEvent *event = sw_event(window, SWTextBatch);
    if (event) {
      event->revision = state ? state->revision : -1;
      if (window->preedit_active)
        sw_text_edit(event, 0, "", 0, 0);
      if (seat->has_delete && (seat->delete_before || seat->delete_after))
        sw_text_edit(event, 2, NULL, seat->delete_before, seat->delete_after);
      if (seat->has_commit && *seat->pending_commit)
        sw_text_edit(event, 1, seat->pending_commit, -1, -1);
      if (seat->has_preedit && *seat->pending_preedit)
        sw_text_edit(event, 0, seat->pending_preedit, seat->pending_begin,
                     seat->pending_end);
      sw_enqueue(window->app, event);
    }
    window->preedit_active = seat->has_preedit && *seat->pending_preedit;
    if (window->preedit_active)
      seat->repeat_deadline = -1;
  }
  pending_reset(seat);
  seat->text_deferred = serial != seat->text_serial;
  if (!seat->text_deferred && seat->text_dirty)
    sw_wayland_text_update(seat);
}

static const struct zwp_text_input_v3_listener text_listener = {
    .enter = text_enter,
    .leave = text_leave,
    .preedit_string = text_preedit,
    .commit_string = text_commit,
    .delete_surrounding_text = text_delete,
    .done = text_done};

void sw_wayland_text_setup(SWSeat *seat) {
  if (!seat->wayland->text_manager || seat->text_input)
    return;
  seat->text_input = zwp_text_input_manager_v3_get_text_input(
      seat->wayland->text_manager, seat->seat);
  zwp_text_input_v3_add_listener(seat->text_input, &text_listener, seat);
}

void sw_wayland_text_destroy(SWSeat *seat) {
  sw_wayland_text_disable(seat);
  if (seat->text_input)
    zwp_text_input_v3_destroy(seat->text_input);
  seat->text_input = NULL;
  seat->text_window = NULL;
}

int sw_wayland_ime(SWWindow *window, bool allowed) {
  if (!window->app->wayland->text_manager)
    return SWUnsupported;
  if (window->ime_allowed == allowed)
    return SWApplied;
  window->ime_allowed = allowed;
  for (SWSeat *seat = window->app->wayland->seats; seat; seat = seat->next) {
    if (seat->text_window != window)
      continue;
    if (allowed)
      sw_wayland_text_update(seat);
    else
      sw_wayland_text_disable(seat);
  }
  if (!allowed)
    sw_cancel_preedit(window);
  sw_emit(window, SWImeState, allowed);
  return SWApplied;
}
