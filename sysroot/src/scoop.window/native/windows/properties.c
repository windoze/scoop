#include "window.h"

static int64_t restyle(SWWindow *window) {
  RECT client;
  GetClientRect(window->hwnd, &client);
  LONG_PTR old = GetWindowLongPtrW(window->hwnd, GWL_STYLE);
  SetLastError(0);
  SetWindowLongPtrW(window->hwnd, GWL_STYLE,
                    sw_style(window) |
                        (old & (WS_VISIBLE | WS_MINIMIZE | WS_MAXIMIZE)));
  DWORD error = GetLastError();
  if (error)
    return error;
  if (!SetWindowPos(window->hwnd, NULL, 0, 0, 0, 0,
                    SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER |
                        SWP_NOACTIVATE))
    return GetLastError();
  return sw_size(window, client.right / window->scale,
                 client.bottom / window->scale);
}

int64_t scoop_window_position(SWWindow *window, double x, double y) {
  if (x < INT_MIN || x > INT_MAX || y < INT_MIN || y > INT_MAX)
    return ERROR_INVALID_PARAMETER;
  return SetWindowPos(window->hwnd, NULL, (int)round(x), (int)round(y), 0, 0,
                      SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE)
             ? 0
             : GetLastError();
}
void sw_minmax(SWWindow *window, MINMAXINFO *info) {
  RECT border = {0};
  AdjustWindowRectExForDpi(&border, sw_style(window), FALSE, 0,
                           (UINT)round(window->scale * 96));
  LONG bw = border.right - border.left, bh = border.bottom - border.top;
  info->ptMinTrackSize.x = (LONG)ceil(window->min_width * window->scale) + bw;
  info->ptMinTrackSize.y = (LONG)ceil(window->min_height * window->scale) + bh;
  if (window->max_width >= 0)
    info->ptMaxTrackSize.x = (LONG)ceil(window->max_width * window->scale) + bw;
  if (window->max_height >= 0)
    info->ptMaxTrackSize.y =
        (LONG)ceil(window->max_height * window->scale) + bh;
}
void sw_sizing(SWWindow *window, WPARAM edge, RECT *rect) {
  RECT border = {0};
  AdjustWindowRectExForDpi(&border, sw_style(window), FALSE, 0,
                           (UINT)round(window->scale * 96));
  double bw = border.right - border.left, bh = border.bottom - border.top;
  double width = (rect->right - rect->left - bw) / window->scale;
  double height = (rect->bottom - rect->top - bh) / window->scale;
  width = round(width / window->increment_width) * window->increment_width;
  height = round(height / window->increment_height) * window->increment_height;
  width = fmax(width, window->min_width);
  height = fmax(height, window->min_height);
  if (window->max_width >= 0)
    width = fmin(width, window->max_width);
  if (window->max_height >= 0)
    height = fmin(height, window->max_height);
  LONG pw = (LONG)ceil(width * window->scale + bw),
       ph = (LONG)ceil(height * window->scale + bh);
  if (edge == WMSZ_LEFT || edge == WMSZ_TOPLEFT || edge == WMSZ_BOTTOMLEFT)
    rect->left = rect->right - pw;
  else
    rect->right = rect->left + pw;
  if (edge == WMSZ_TOP || edge == WMSZ_TOPLEFT || edge == WMSZ_TOPRIGHT)
    rect->top = rect->bottom - ph;
  else
    rect->bottom = rect->top + ph;
}
int64_t scoop_window_constraints(SWWindow *window, double minw, double minh,
                                 double maxw, double maxh) {
  if (minw * window->scale > INT_MAX / 2 ||
      minh * window->scale > INT_MAX / 2 ||
      maxw * window->scale > INT_MAX / 2 || maxh * window->scale > INT_MAX / 2)
    return ERROR_INVALID_PARAMETER;
  window->min_width = minw;
  window->min_height = minh;
  window->max_width = maxw;
  window->max_height = maxh;
  SWSnapshot value = sw_snapshot(window);
  double width = fmax(value.width, minw), height = fmax(value.height, minh);
  if (maxw >= 0)
    width = fmin(width, maxw);
  if (maxh >= 0)
    height = fmin(height, maxh);
  return sw_size(window, width, height);
}
int64_t scoop_window_resize_increments(SWWindow *window, double width,
                                       double height) {
  window->increment_width = width;
  window->increment_height = height;
  return 0;
}
int64_t scoop_window_resizable(SWWindow *window, int32_t value) {
  window->resizable = value != 0;
  return restyle(window);
}
int64_t scoop_window_decorations(SWWindow *window, int32_t value) {
  window->decorations = value != 0;
  return restyle(window);
}
int64_t scoop_window_buttons(SWWindow *window, int32_t close, int32_t minimize,
                             int32_t maximize) {
  window->close_button = close != 0;
  window->minimize_button = minimize != 0;
  window->maximize_button = maximize != 0;
  HMENU menu = GetSystemMenu(window->hwnd, FALSE);
  if (menu)
    EnableMenuItem(menu, SC_CLOSE,
                   MF_BYCOMMAND | (close ? MF_ENABLED : MF_GRAYED));
  return restyle(window);
}
int64_t scoop_window_level(SWWindow *window, int32_t level) {
  window->bottom = level < 0;
  HWND order = level > 0   ? HWND_TOPMOST
               : level < 0 ? HWND_BOTTOM
                           : HWND_NOTOPMOST;
  return SetWindowPos(window->hwnd, order, 0, 0, 0, 0,
                      SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE)
             ? 0
             : GetLastError();
}
int64_t scoop_window_minimized(SWWindow *window, int32_t value) {
  ShowWindow(window->hwnd, value ? SW_MINIMIZE : SW_RESTORE);
  return 0;
}
int64_t scoop_window_maximized(SWWindow *window, int32_t value) {
  ShowWindow(window->hwnd, value ? SW_MAXIMIZE : SW_RESTORE);
  return 0;
}
int sw_theme(SWWindow *window) {
  if (window->theme >= 0)
    return window->theme;
  DWORD light = 1, size = sizeof(light);
  RegGetValueW(
      HKEY_CURRENT_USER,
      L"Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize",
      L"AppsUseLightTheme", RRF_RT_REG_DWORD, NULL, &light, &size);
  return light == 0;
}
int64_t scoop_window_theme(SWWindow *window, int32_t theme) {
  int previous = window->theme;
  window->theme = theme;
  BOOL dark = sw_theme(window);
  HRESULT hr = DwmSetWindowAttribute(window->hwnd, 20, &dark, sizeof(dark));
  if (FAILED(hr)) {
    window->theme = previous;
    return hr == E_INVALIDARG ? -1 : (uint32_t)hr;
  }
  sw_emit(window, 6);
  return 0;
}
int64_t scoop_window_transparent(SWWindow *window, int32_t value) {
  int margin = value ? -1 : 0;
  MARGINS margins = {margin, margin, margin, margin};
  HRESULT hr = DwmExtendFrameIntoClientArea(window->hwnd, &margins);
  if (FAILED(hr))
    return (uint32_t)hr;
  window->transparent = value != 0;
  return 0;
}
int64_t scoop_window_blur(SWWindow *window, int32_t value) {
  // DwmEnableBlurBehindWindow no longer produces blur on Windows 8 and later.
  // Undocumented composition attributes are not a supported capability.
  return value ? -1 : 0;
}
int64_t scoop_window_content_protection(SWWindow *window, int32_t value) {
  return SetWindowDisplayAffinity(window->hwnd, value ? WDA_MONITOR : WDA_NONE)
             ? 0
             : GetLastError();
}
int64_t scoop_window_attention(SWWindow *window, int32_t value) {
  FLASHWINFO info = {
      .cbSize = sizeof(info),
      .hwnd = window->hwnd,
      .dwFlags = value < 0 ? FLASHW_STOP
                           : FLASHW_ALL | (value > 0 ? FLASHW_TIMERNOFG : 0),
      .uCount = value > 0 ? UINT_MAX : 3};
  FlashWindowEx(&info);
  return 0;
}
