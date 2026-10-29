#include "window.h"

uint64_t sw_modifiers(void) {
  uint64_t value = 0;
  if (GetKeyState(VK_SHIFT) & 0x8000)
    value |= 1;
  if (GetKeyState(VK_CONTROL) & 0x8000)
    value |= 2;
  if (GetKeyState(VK_MENU) & 0x8000)
    value |= 4;
  if ((GetKeyState(VK_LWIN) | GetKeyState(VK_RWIN)) & 0x8000)
    value |= 8;
  if (GetKeyState(VK_CAPITAL) & 1)
    value |= 16;
  if (GetKeyState(VK_NUMLOCK) & 1)
    value |= 32;
  if (GetKeyState(VK_SCROLL) & 1)
    value |= 64;
  const int keys[] = {VK_LSHIFT, VK_RSHIFT, VK_LCONTROL, VK_RCONTROL,
                      VK_LMENU,  VK_RMENU,  VK_LWIN,     VK_RWIN};
  for (int n = 0; n < 8; ++n)
    if (GetKeyState(keys[n]) & 0x8000)
      value |= UINT64_C(256) << n;
  return value;
}

static void modifiers(SWWindow *window, uint64_t value) {
  if (value == window->modifiers)
    return;
  window->modifiers = value;
  SWEvent *event = sw_event(window, 11);
  if (event) {
    event->data.a = (int64_t)value;
    sw_enqueue(window->application, event);
  }
}

static void key(SWWindow *window, UINT msg, WPARAM vk, LPARAM lp) {
  UINT scan = (UINT)((lp >> 16) & 0xff);
  UINT code = scan | ((lp & (1LL << 24)) ? 0x100 : 0);
  bool down = msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN;
  uint64_t mods = sw_modifiers();
  modifiers(window, mods);
  if (vk == VK_PROCESSKEY || (down && window->composing))
    return;
  if (!down && !window->key_down[code])
    return;
  SWEvent *event = sw_event(window, 10);
  if (!event)
    return;
  event->data.a = code;
  event->data.b = (int64_t)mods;
  event->data.c = (int64_t)vk;
  event->data.flags = (down ? 1 : 0) | (down && (lp & (1LL << 30)) ? 2 : 0);
  if (down) {
    BYTE state[256];
    WCHAR text[8];
    GetKeyboardState(state);
    if (!((state[VK_RMENU] & 0x80) && (state[VK_CONTROL] & 0x80)))
      state[VK_CONTROL] = state[VK_LCONTROL] = state[VK_RCONTROL] = 0;
    // Flag 4 preserves the system dead-key state; TranslateMessage owns
    // composition.
    int count =
        ToUnicodeEx((UINT)vk, scan, state, text, 8, 4, GetKeyboardLayout(0));
    if (count < 0) {
      event->data.flags |= 8;
      count = -count;
    }
    if (count > 0 && text[0] >= 0x20)
      event->data.text = sw_utf8(text, count, &event->data.text_length);
    free((void *)window->pressed[code].text);
    window->pressed[code] = event->data;
    if (event->data.text) {
      uint8_t *copy = malloc((size_t)event->data.text_length + 1);
      if (!copy) {
        sw_free_event(event);
        window->pressed[code].text = NULL;
        sw_fail(window->application, ERROR_NOT_ENOUGH_MEMORY);
        return;
      }
      memcpy(copy, event->data.text, (size_t)event->data.text_length + 1);
      window->pressed[code].text = copy;
    }
    window->key_down[code] = true;
  } else {
    event->data = window->pressed[code];
    event->data.flags &= 8;
    event->data.b = (int64_t)mods;
    window->pressed[code].text = NULL;
    window->key_down[code] = false;
  }
  sw_enqueue(window->application, event);
}

void sw_reset_input(SWWindow *window) {
  sw_cancel_composition(window);
  uint64_t locks = sw_modifiers() & 112;
  for (int n = 0; n < 512; ++n) {
    if (!window->key_down[n])
      continue;
    SWEvent *event = sw_event(window, 10);
    if (event) {
      event->data = window->pressed[n];
      event->data.flags = (event->data.flags & 8) | 4;
      event->data.b = (int64_t)locks;
      window->pressed[n].text = NULL;
      sw_enqueue(window->application, event);
    }
    window->key_down[n] = false;
  }
  modifiers(window, locks);
  window->surrogate = 0;
}

static void pointer(SWWindow *window, int kind, POINT position, int64_t button,
                    int clicks, double dx, double dy) {
  SWEvent *event = sw_event(window, 12);
  if (!event)
    return;
  event->data.flags = kind;
  event->data.a = button;
  event->data.c = clicks;
  event->data.b = (int64_t)sw_modifiers();
  event->data.scale = GetDpiForWindow(window->hwnd) / 96.0;
  event->data.z = position.x;
  event->data.w = position.y;
  event->data.x = position.x / event->data.scale;
  event->data.y = position.y / event->data.scale;
  event->data.dx = dx;
  event->data.dy = dy;
  sw_enqueue(window->application, event);
}

bool sw_input(SWWindow *window, UINT msg, WPARAM wp, LPARAM lp,
              LRESULT *result) {
  *result = 0;
  POINT position = {GET_X_LPARAM(lp), GET_Y_LPARAM(lp)};
  switch (msg) {
  case WM_KEYDOWN:
  case WM_KEYUP:
  case WM_SYSKEYDOWN:
  case WM_SYSKEYUP:
    key(window, msg, wp, lp);
    // DefWindowProc must still implement system shortcuts such as Alt+F4.
    return false;
  case WM_SETCURSOR:
    if (LOWORD(lp) == HTCLIENT) {
      SetCursor(window->cursor_visible ? window->cursor : NULL);
      *result = TRUE;
      return true;
    }
    return false;
  case WM_MOUSEMOVE:
    if (!window->pointer_inside) {
      window->pointer_inside = true;
      TRACKMOUSEEVENT track = {.cbSize = sizeof(track),
                               .dwFlags = TME_LEAVE,
                               .hwndTrack = window->hwnd};
      if (!TrackMouseEvent(&track))
        sw_fail(window->application, GetLastError());
      pointer(window, 0, position, 0, 0, 0, 0);
    }
    pointer(window, 2, position, 0, 0, 0, 0);
    return true;
  case WM_MOUSELEAVE:
    window->pointer_inside = false;
    GetCursorPos(&position);
    ScreenToClient(window->hwnd, &position);
    pointer(window, 1, position, 0, 0, 0, 0);
    return true;
  case WM_LBUTTONDOWN:
  case WM_RBUTTONDOWN:
  case WM_MBUTTONDOWN:
  case WM_XBUTTONDOWN:
  case WM_LBUTTONDBLCLK:
  case WM_RBUTTONDBLCLK:
  case WM_MBUTTONDBLCLK:
  case WM_XBUTTONDBLCLK:
  case WM_LBUTTONUP:
  case WM_RBUTTONUP:
  case WM_MBUTTONUP:
  case WM_XBUTTONUP: {
    bool up = msg == WM_LBUTTONUP || msg == WM_RBUTTONUP ||
              msg == WM_MBUTTONUP || msg == WM_XBUTTONUP;
    bool twice = msg == WM_LBUTTONDBLCLK || msg == WM_RBUTTONDBLCLK ||
                 msg == WM_MBUTTONDBLCLK || msg == WM_XBUTTONDBLCLK;
    int button = msg >= WM_XBUTTONDOWN   ? 2 + GET_XBUTTON_WPARAM(wp)
                 : msg >= WM_MBUTTONDOWN ? 2
                 : msg >= WM_RBUTTONDOWN ? 1
                                         : 0;
    if (!up)
      SetCapture(window->hwnd);
    else if (!(LOWORD(wp) & (MK_LBUTTON | MK_RBUTTON | MK_MBUTTON |
                             MK_XBUTTON1 | MK_XBUTTON2)) &&
             GetCapture() == window->hwnd)
      ReleaseCapture();
    pointer(window, up ? 4 : 3, position, button, twice ? 2 : 1, 0, 0);
    *result = TRUE;
    return true;
  }
  case WM_MOUSEWHEEL:
  case WM_MOUSEHWHEEL: {
    ScreenToClient(window->hwnd, &position);
    double delta = GET_WHEEL_DELTA_WPARAM(wp) / (double)WHEEL_DELTA;
    pointer(window, 5, position, 0, 0, msg == WM_MOUSEHWHEEL ? delta : 0,
            msg == WM_MOUSEWHEEL ? delta : 0);
    return true;
  }
  default:
    return false;
  }
}

int64_t scoop_window_cursor_icon(SWWindow *window, int32_t icon) {
  const WCHAR *names[] = {IDC_ARROW, IDC_IBEAM,  IDC_CROSS,
                          IDC_HAND,  IDC_HAND,   IDC_SIZEALL,
                          IDC_NO,    IDC_SIZEWE, IDC_SIZENS};
  HCURSOR cursor = LoadCursorW(NULL, names[icon]);
  if (!cursor)
    return GetLastError();
  window->cursor = cursor;
  if (window->pointer_inside)
    SetCursor(window->cursor_visible ? cursor : NULL);
  return 0;
}
int64_t scoop_window_cursor_visible(SWWindow *window, int32_t visible) {
  window->cursor_visible = visible != 0;
  if (window->pointer_inside)
    SetCursor(visible ? window->cursor : NULL);
  return 0;
}
