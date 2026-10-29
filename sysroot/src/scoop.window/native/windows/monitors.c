#include "window.h"
#include <shellscalingapi.h>

typedef struct {
  int64_t key, primary, refresh_millihertz;
  double x, y, width, height, pixel_width, pixel_height, scale;
  const uint8_t *name;
  int64_t name_length;
} SWMonitor;
typedef struct {
  SWMonitor *items;
  int64_t count;
  DWORD error;
} SWMonitors;

static BOOL CALLBACK enumerate(HMONITOR monitor, HDC dc, LPRECT bounds,
                               LPARAM context) {
  SWMonitors *snapshot = (SWMonitors *)context;
  MONITORINFOEXW info = {.cbSize = sizeof(info)};
  if (!GetMonitorInfoW(monitor, (MONITORINFO *)&info)) {
    snapshot->error = GetLastError();
    return FALSE;
  }
  SWMonitor *items = realloc(snapshot->items,
                             ((size_t)snapshot->count + 1) * sizeof(SWMonitor));
  if (!items) {
    snapshot->error = ERROR_NOT_ENOUGH_MEMORY;
    return FALSE;
  }
  snapshot->items = items;
  SWMonitor value = {0};
  value.key = (int64_t)(intptr_t)monitor;
  value.primary = (info.dwFlags & MONITORINFOF_PRIMARY) != 0;
  value.x = info.rcMonitor.left;
  value.y = info.rcMonitor.top;
  value.width = value.pixel_width = info.rcMonitor.right - info.rcMonitor.left;
  value.height = value.pixel_height =
      info.rcMonitor.bottom - info.rcMonitor.top;
  UINT x = 96, y = 96;
  HRESULT hr = GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &x, &y);
  if (FAILED(hr)) {
    snapshot->error = (DWORD)hr;
    return FALSE;
  }
  value.scale = x / 96.0;
  DEVMODEW mode = {.dmSize = sizeof(mode)};
  if (EnumDisplaySettingsW(info.szDevice, ENUM_CURRENT_SETTINGS, &mode) &&
      mode.dmDisplayFrequency > 1)
    value.refresh_millihertz = (int64_t)mode.dmDisplayFrequency * 1000;
  DISPLAY_DEVICEW device = {.cb = sizeof(device)};
  const WCHAR *name = info.szDevice;
  if (EnumDisplayDevicesW(info.szDevice, 0, &device, 0))
    name = device.DeviceString;
  value.name = sw_utf8(name, (int)wcslen(name), &value.name_length);
  if (!value.name) {
    snapshot->error = GetLastError();
    return FALSE;
  }
  snapshot->items[snapshot->count++] = value;
  return TRUE;
}

void scoop_window_monitors_release(SWMonitors *snapshot) {
  for (int64_t n = 0; n < snapshot->count; ++n)
    free((void *)snapshot->items[n].name);
  free(snapshot->items);
  free(snapshot);
}
void *scoop_window_monitors(void) {
  SWMonitors *snapshot = calloc(1, sizeof(*snapshot));
  if (!snapshot)
    return NULL;
  if (!EnumDisplayMonitors(NULL, NULL, enumerate, (LPARAM)snapshot)) {
    scoop_window_monitors_release(snapshot);
    return NULL;
  }
  return snapshot;
}
int64_t scoop_window_monitors_count(SWMonitors *snapshot) {
  return snapshot->count;
}
SWMonitor scoop_window_monitor_data(SWMonitors *snapshot, int64_t index) {
  return snapshot->items[index];
}
int64_t scoop_window_current_monitor(SWWindow *window) {
  return (int64_t)(intptr_t)MonitorFromWindow(window->hwnd,
                                              MONITOR_DEFAULTTONULL);
}
