#include "window.h"

static bool append(SWEvent *event, int kind, const WCHAR *text, int count,
                   int cursor) {
  SWTextEditData *edit = &event->edits[event->edit_count];
  edit->kind = kind;
  edit->anchor = edit->cursor = -1;
  edit->text = sw_utf8(text, count, &edit->text_length);
  if (!edit->text)
    return false;
  ++event->edit_count;
  if (cursor >= 0) {
    if (cursor > count ||
        (cursor > 0 && cursor < count && text[cursor - 1] >= 0xD800 &&
         text[cursor - 1] <= 0xDBFF && text[cursor] >= 0xDC00 &&
         text[cursor] <= 0xDFFF))
      return false;
    int bytes = cursor ? WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS,
                                             text, cursor, NULL, 0, NULL, NULL)
                       : 0;
    if (cursor && !bytes)
      return false;
    edit->anchor = edit->cursor = bytes;
  }
  return true;
}

static void clear_preedit(SWWindow *window) {
  if (!window->marked)
    return;
  window->marked = false;
  SWEvent *event = sw_event(window, 13);
  if (!event)
    return;
  if (!append(event, 0, L"", 0, 0)) {
    sw_free_event(event);
    sw_fail(window->application, ERROR_NOT_ENOUGH_MEMORY);
    return;
  }
  sw_enqueue(window->application, event);
}

void sw_cancel_composition(SWWindow *window) {
  if (window->ime && window->composing)
    ImmNotifyIME(window->ime, NI_COMPOSITIONSTR, CPS_CANCEL, 0);
  window->composing = false;
  clear_preedit(window);
}

void sw_update_ime_caret(SWWindow *window) {
  if (!window->ime)
    return;
  double scale = window->scale;
  POINT point = {(LONG)round(window->caret_x * scale),
                 (LONG)round(window->caret_y * scale)};
  RECT rect = {point.x, point.y,
               (LONG)ceil((window->caret_x + window->caret_width) * scale),
               (LONG)ceil((window->caret_y + window->caret_height) * scale)};
  window->caret = rect;
  CANDIDATEFORM candidate = {.dwIndex = 0,
                             .dwStyle = CFS_EXCLUDE,
                             .ptCurrentPos = point,
                             .rcArea = rect};
  COMPOSITIONFORM composition = {.dwStyle = CFS_POINT, .ptCurrentPos = point};
  ImmSetCandidateWindow(window->ime, &candidate);
  ImmSetCompositionWindow(window->ime, &composition);
}

int32_t scoop_window_ime_allowed(SWWindow *window, int32_t allowed) {
  if (window->ime_allowed == (allowed != 0))
    return 0;
  if (allowed && !window->ime) {
    window->ime = ImmCreateContext();
    if (!window->ime)
      return 1;
  }
  if (!allowed)
    sw_cancel_composition(window);
  window->ime_allowed = allowed != 0;
  ImmAssociateContext(window->hwnd, allowed ? window->ime : NULL);
  sw_update_ime_caret(window);
  SWEvent *event = sw_event(window, 14);
  if (event) {
    event->data.flags = allowed != 0;
    sw_enqueue(window->application, event);
  }
  return 0;
}
int64_t scoop_window_ime_caret(SWWindow *window, double x, double y,
                               double width, double height) {
  if (fabs(x * window->scale) > INT_MAX / 2 ||
      fabs(y * window->scale) > INT_MAX / 2 ||
      fabs((x + width) * window->scale) > INT_MAX / 2 ||
      fabs((y + height) * window->scale) > INT_MAX / 2)
    return ERROR_INVALID_PARAMETER;
  window->caret_x = x;
  window->caret_y = y;
  window->caret_width = width;
  window->caret_height = height;
  sw_update_ime_caret(window);
  return 0;
}

typedef struct {
  WCHAR *text;
  int count, cursor;
} CompositionPart;

static bool read_composition(SWWindow *window, DWORD part,
                             CompositionPart *value) {
  LONG bytes = ImmGetCompositionStringW(window->ime, part, NULL, 0);
  if (bytes < 0 || bytes % 2 != 0)
    return false;
  value->text = calloc((size_t)bytes / 2 + 1, sizeof(WCHAR));
  if (!value->text) {
    sw_fail(window->application, ERROR_NOT_ENOUGH_MEMORY);
    return false;
  }
  value->count = bytes / 2;
  value->cursor =
      part == GCS_COMPSTR
          ? ImmGetCompositionStringW(window->ime, GCS_CURSORPOS, NULL, 0)
          : -1;
  return ImmGetCompositionStringW(window->ime, part, value->text,
                                  (DWORD)bytes) == bytes;
}

// Normalize one native UTF-16 batch before publishing any of its edits.
void sw_composition(SWWindow *window, const WCHAR *commit, int commit_count,
                    const WCHAR *preedit, int preedit_count, int cursor) {
  SWEvent *event = sw_event(window, 13);
  if (!event)
    return;
  if ((commit && !append(event, 1, commit, commit_count, -1)) ||
      (preedit && !append(event, 0, preedit, preedit_count, cursor))) {
    sw_free_event(event);
    sw_cancel_composition(window);
    return;
  }
  if (commit)
    window->marked = false;
  if (preedit)
    window->marked = preedit_count > 0;
  if (event->edit_count)
    sw_enqueue(window->application, event);
  else
    sw_free_event(event);
}

static void commit_character(SWWindow *window, WCHAR character) {
  if (character >= 0xD800 && character <= 0xDBFF) {
    window->surrogate = character;
    return;
  }
  WCHAR text[2] = {character, 0};
  int length = 1;
  if (character >= 0xDC00 && character <= 0xDFFF) {
    if (!window->surrogate)
      return;
    text[0] = window->surrogate;
    text[1] = character;
    length = 2;
  }
  window->surrogate = 0;
  if (character < 0x20 || character == 0x7f)
    return;
  SWEvent *event = sw_event(window, 13);
  if (!event)
    return;
  if (!append(event, 1, text, length, -1)) {
    sw_free_event(event);
    return;
  }
  sw_enqueue(window->application, event);
}

bool sw_text(SWWindow *window, UINT msg, WPARAM wp, LPARAM lp,
             LRESULT *result) {
  *result = 0;
  switch (msg) {
  case WM_IME_SETCONTEXT:
    *result = DefWindowProcW(window->hwnd, msg, wp,
                             lp & ~ISC_SHOWUICOMPOSITIONWINDOW);
    return true;
  case WM_IME_STARTCOMPOSITION:
    if (window->ime_allowed) {
      window->composing = true;
      sw_update_ime_caret(window);
    }
    return true;
  case WM_IME_ENDCOMPOSITION:
    window->composing = false;
    clear_preedit(window);
    return true;
  case WM_IME_COMPOSITION: {
    if (!window->ime_allowed)
      return true;
    // CS_INSERTCHAR is an interim composition, never a second commit channel.
    if (lp & CS_INSERTCHAR) {
      WCHAR character = (WCHAR)wp;
      sw_composition(window, NULL, 0, &character, 1,
                     lp & CS_NOMOVECARET ? 0 : 1);
      return true;
    }
    const LPARAM changes = GCS_COMPREADSTR | GCS_COMPREADATTR |
                           GCS_COMPREADCLAUSE | GCS_COMPSTR | GCS_COMPATTR |
                           GCS_COMPCLAUSE | GCS_CURSORPOS | GCS_DELTASTART |
                           GCS_RESULTREADSTR | GCS_RESULTREADCLAUSE |
                           GCS_RESULTSTR | GCS_RESULTCLAUSE;
    if (!(lp & changes)) {
      clear_preedit(window);
      return true;
    }
    CompositionPart commit = {0}, preedit = {0};
    bool valid = !(lp & GCS_RESULTSTR) ||
                 read_composition(window, GCS_RESULTSTR, &commit);
    if (valid && (lp & (GCS_COMPSTR | GCS_CURSORPOS)))
      valid = read_composition(window, GCS_COMPSTR, &preedit);
    if (valid)
      sw_composition(window, commit.text, commit.count, preedit.text,
                     preedit.count, preedit.cursor);
    else
      sw_cancel_composition(window);
    free(commit.text);
    free(preedit.text);
    return true;
  }
  case WM_IME_CHAR:
    return true; // GCS_RESULTSTR is the sole IME commit path.
  case WM_CHAR:
    if (!window->composing)
      commit_character(window, (WCHAR)wp);
    return true;
  case WM_UNICHAR:
    if (wp == UNICODE_NOCHAR) {
      *result = TRUE;
      return true;
    }
    if (wp > 0x10FFFF || (wp >= 0xD800 && wp <= 0xDFFF))
      return true;
    if (wp >= 0x10000) {
      commit_character(window, (WCHAR)(0xD800 + ((wp - 0x10000) >> 10)));
      commit_character(window, (WCHAR)(0xDC00 + ((wp - 0x10000) & 1023)));
    } else
      commit_character(window, (WCHAR)wp);
    return true;
  case WM_SYSCHAR:
  case WM_DEADCHAR:
  case WM_SYSDEADCHAR:
    return true;
  case WM_INPUTLANGCHANGE:
    sw_cancel_composition(window);
    return false;
  default:
    return false;
  }
}

int64_t scoop_window_text_count(SWEvent *event) { return event->edit_count; }
SWTextEditData scoop_window_text_edit(SWEvent *event, int64_t index) {
  return event->edits[index];
}
