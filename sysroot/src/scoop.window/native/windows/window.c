#include "window.h"

DWORD sw_style(SWWindow *window) {
  DWORD style = WS_CLIPCHILDREN | WS_CLIPSIBLINGS;
  style |= window->decorations ? WS_CAPTION | WS_SYSMENU : WS_POPUP;
  if (window->resizable)
    style |= WS_THICKFRAME;
  if (window->minimize_button)
    style |= WS_MINIMIZEBOX;
  if (window->maximize_button && window->resizable)
    style |= WS_MAXIMIZEBOX;
  return style;
}

SWSnapshot sw_snapshot(SWWindow *window) {
  RECT client = {0}, outer = {0};
  GetClientRect(window->hwnd, &client);
  GetWindowRect(window->hwnd, &outer);
  SWSnapshot result = {0};
  result.scale = GetDpiForWindow(window->hwnd) / 96.0;
  if (result.scale <= 0)
    result.scale = window->scale;
  result.minimized = IsIconic(window->hwnd);
  result.pixel_width = result.minimized ? 0 : client.right;
  result.pixel_height = result.minimized ? 0 : client.bottom;
  result.width = result.pixel_width / result.scale;
  result.height = result.pixel_height / result.scale;
  result.x = outer.left;
  result.y = outer.top;
  result.outer_width = outer.right - outer.left;
  result.outer_height = outer.bottom - outer.top;
  result.visible = IsWindowVisible(window->hwnd);
  DWORD cloaked = 0;
  DwmGetWindowAttribute(window->hwnd, DWMWA_CLOAKED, &cloaked, sizeof(cloaked));
  result.occluded = result.minimized || cloaked != 0;
  result.focused = GetFocus() == window->hwnd;
  result.maximized = IsZoomed(window->hwnd);
  result.dark = sw_theme(window);
  return result;
}

void sw_emit(SWWindow *window, int32_t kind) {
  if (window->destroying)
    return;
  SWEvent *event = sw_event(window, kind);
  if (!event)
    return;
  SWSnapshot snapshot = sw_snapshot(window);
  switch (kind) {
  case 2:
    event->data.x = snapshot.width;
    event->data.y = snapshot.height;
    event->data.z = snapshot.pixel_width;
    event->data.w = snapshot.pixel_height;
    event->data.scale = snapshot.scale;
    break;
  case 3:
    event->data.x = snapshot.x;
    event->data.y = snapshot.y;
    break;
  case 4:
    event->data.flags = snapshot.focused;
    break;
  case 5:
    event->data.flags = snapshot.occluded;
    break;
  case 6:
    event->data.flags = snapshot.dark;
    break;
  default:
    break;
  }
  sw_enqueue(window->application, event);
}

LRESULT CALLBACK sw_window_proc(HWND hwnd, UINT msg, WPARAM wp, LPARAM lp) {
  SWWindow *window = (SWWindow *)GetWindowLongPtrW(hwnd, GWLP_USERDATA);
  if (msg == WM_NCCREATE) {
    window = ((CREATESTRUCTW *)lp)->lpCreateParams;
    window->hwnd = hwnd;
    SetWindowLongPtrW(hwnd, GWLP_USERDATA, (LONG_PTR)window);
  }
  if (!window)
    return DefWindowProcW(hwnd, msg, wp, lp);
  LRESULT result;
  if (!window->destroying && (sw_text(window, msg, wp, lp, &result) ||
                              sw_input(window, msg, wp, lp, &result)))
    return result;
  switch (msg) {
  case WM_CLOSE:
    if (window->close_button)
      sw_emit(window, 1);
    return 0;
  case WM_QUERYENDSESSION:
    // The hidden application window receives the single process-level intent.
    return FALSE;
  case WM_SIZE:
    sw_emit(window, 2);
    sw_emit(window, 5);
    return 0;
  case WM_MOVE:
    sw_emit(window, 3);
    return 0;
  case WM_SETFOCUS:
    sw_emit(window, 4);
    return 0;
  case WM_KILLFOCUS:
    sw_reset_input(window);
    sw_emit(window, 4);
    return 0;
  case WM_SHOWWINDOW:
    sw_wake(window->application);
    break;
  case WM_PAINT: {
    PAINTSTRUCT paint;
    BeginPaint(hwnd, &paint);
    EndPaint(hwnd, &paint);
    sw_emit(window, 15);
    return 0;
  }
  case WM_ERASEBKGND:
    return 1;
  case WM_GETMINMAXINFO:
    sw_minmax(window, (MINMAXINFO *)lp);
    return 0;
  case WM_SIZING:
    sw_sizing(window, wp, (RECT *)lp);
    return TRUE;
  case WM_DPICHANGED: {
    window->scale = HIWORD(wp) / 96.0;
    RECT *rect = (RECT *)lp;
    if (!SetWindowPos(hwnd, NULL, rect->left, rect->top,
                      rect->right - rect->left, rect->bottom - rect->top,
                      SWP_NOZORDER | SWP_NOACTIVATE))
      sw_fail(window->application, GetLastError());
    sw_update_ime_caret(window);
    sw_emit(window, 2);
    return 0;
  }
  case WM_WINDOWPOSCHANGING:
    if (window->bottom) {
      WINDOWPOS *position = (WINDOWPOS *)lp;
      if (!(position->flags & SWP_NOZORDER))
        position->hwndInsertAfter = HWND_BOTTOM;
    }
    break;
  case WM_SETTINGCHANGE:
  case WM_THEMECHANGED:
    sw_emit(window, 6);
    break;
  case WM_NCDESTROY:
    SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
    break;
  default:
    break;
  }
  return DefWindowProcW(hwnd, msg, wp, lp);
}

SWResult scoop_window_create(SWApplication *app, int64_t identity,
                             const uint8_t *title, int64_t length, double width,
                             double height, int32_t flags) {
  SWWindow *window = calloc(1, sizeof(*window));
  if (!window)
    return (SWResult){NULL, ERROR_NOT_ENOUGH_MEMORY};
  window->application = app;
  window->identity = identity;
  window->decorations = (flags & 1) != 0;
  window->resizable = (flags & 2) != 0;
  window->transparent = (flags & 4) != 0;
  window->close_button = window->minimize_button = window->maximize_button =
      true;
  window->cursor_visible = true;
  window->cursor = LoadCursorW(NULL, IDC_ARROW);
  window->max_width = window->max_height = -1;
  window->theme = -1;
  window->increment_width = window->increment_height = 1;
  window->scale = GetDpiForSystem() / 96.0;
  WCHAR *text = sw_wide(title, length);
  DWORD error = 0;
  if (!text)
    error = GetLastError();
  else {
    window->hwnd = CreateWindowExW(0, sw_window_class, text, sw_style(window),
                                   CW_USEDEFAULT, CW_USEDEFAULT, 0, 0, NULL,
                                   NULL, GetModuleHandleW(NULL), window);
    free(text);
    if (!window->hwnd)
      error = GetLastError();
    else {
      window->scale = GetDpiForWindow(window->hwnd) / 96.0;
      ImmAssociateContext(window->hwnd, NULL);
      error = (DWORD)sw_size(window, width, height);
      if (!error && window->transparent) {
        MARGINS margins = {-1, -1, -1, -1};
        HRESULT hr = DwmExtendFrameIntoClientArea(window->hwnd, &margins);
        if (FAILED(hr))
          error = (DWORD)hr;
      }
    }
  }
  if (!error)
    return (SWResult){window, 0};
  window->destroying = true;
  if (window->hwnd)
    DestroyWindow(window->hwnd);
  free(window);
  return (SWResult){NULL, error};
}

SWSnapshot scoop_window_snapshot(SWWindow *window) {
  return sw_snapshot(window);
}
void scoop_window_destroy(SWWindow *window) {
  window->destroying = true;
  ImmAssociateContext(window->hwnd, NULL);
  if (window->ime)
    ImmDestroyContext(window->ime);
  DestroyWindow(window->hwnd);
  for (int n = 0; n < 512; ++n)
    free((void *)window->pressed[n].text);
  free(window);
}
void *scoop_window_hwnd(SWWindow *window) { return window->hwnd; }
void *scoop_window_hinstance(SWWindow *window) {
  return GetModuleHandleW(NULL);
}

int64_t scoop_window_title(SWWindow *window, const uint8_t *title,
                           int64_t length) {
  WCHAR *text = sw_wide(title, length);
  if (!text)
    return GetLastError();
  BOOL ok = SetWindowTextW(window->hwnd, text);
  DWORD error = ok ? 0 : GetLastError();
  free(text);
  return error;
}
int64_t sw_size(SWWindow *window, double width, double height) {
  double pw = ceil(width * window->scale), ph = ceil(height * window->scale);
  if (pw > INT_MAX / 2 || ph > INT_MAX / 2)
    return ERROR_INVALID_PARAMETER;
  RECT rect = {0, 0, (LONG)pw, (LONG)ph};
  if (!AdjustWindowRectExForDpi(&rect, sw_style(window), FALSE, 0,
                                (UINT)round(window->scale * 96)))
    return GetLastError();
  return SetWindowPos(window->hwnd, NULL, 0, 0, rect.right - rect.left,
                      rect.bottom - rect.top,
                      SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE)
             ? 0
             : GetLastError();
}
int64_t scoop_window_size(SWWindow *window, double width, double height) {
  return sw_size(window, width, height);
}
int64_t scoop_window_visible(SWWindow *window, int32_t visible) {
  ShowWindow(window->hwnd, visible ? SW_SHOWNOACTIVATE : SW_HIDE);
  sw_wake(window->application);
  return 0;
}
int64_t scoop_window_focus(SWWindow *window) {
  // The foreground lock can deny activation; the public result remains
  // Requested.
  SetForegroundWindow(window->hwnd);
  return 0;
}
