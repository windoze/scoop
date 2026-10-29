#include "x11.h"
#include <X11/Xresource.h>

static double display_scale(SWX11 *x) {
  const char *resources = XResourceManagerString(x->display);
  if (!resources)
    return 1;
  XrmDatabase database = XrmGetStringDatabase(resources);
  if (!database)
    return 1;
  XrmValue value;
  char *type;
  double scale = 1;
  if (XrmGetResource(database, "Xft.dpi", "Xft.Dpi", &type, &value) &&
      value.addr) {
    char *end;
    double dpi = strtod(value.addr, &end);
    if (end != value.addr && isfinite(dpi) && dpi > 0)
      scale = dpi / 96.0;
  }
  XrmDestroyDatabase(database);
  return scale;
}

static int64_t monitor_key(SWApp *app, uint64_t native) {
  for (SWMonitor *monitor = app->monitors; monitor; monitor = monitor->next)
    if (monitor->native_id == native)
      return monitor->data.key;
  if (app->next_monitor == INT64_MAX) {
    sw_error(app, SWErrorNative, EOVERFLOW);
    app->error_pending = true;
    return 0;
  }
  return app->next_monitor++;
}

static int64_t refresh_rate(SWX11 *x, XRRScreenResources *resources,
                            XRRMonitorInfo *monitor) {
  if (!resources)
    return 0;
  for (int output = 0; output < monitor->noutput; ++output) {
    XRROutputInfo *info =
        XRRGetOutputInfo(x->display, resources, monitor->outputs[output]);
    if (!info)
      continue;
    RRCrtc crtc = info->crtc;
    XRRFreeOutputInfo(info);
    if (!crtc)
      continue;
    XRRCrtcInfo *current = XRRGetCrtcInfo(x->display, resources, crtc);
    if (!current)
      continue;
    RRMode mode = current->mode;
    XRRFreeCrtcInfo(current);
    for (int i = 0; i < resources->nmode; ++i) {
      XRRModeInfo *value = &resources->modes[i];
      if (value->id != mode || !value->hTotal || !value->vTotal)
        continue;
      double rate =
          (double)value->dotClock * 1000.0 / value->hTotal / value->vTotal;
      if (value->modeFlags & RR_Interlace)
        rate *= 2;
      if (value->modeFlags & RR_DoubleScan)
        rate /= 2;
      return rate > 0 && rate < (double)INT64_MAX ? (int64_t)llround(rate) : 0;
    }
  }
  return 0;
}

void sw_x11_monitors(SWX11 *x) {
  SWApp *app = x->app;
  double old_scale = x->scale;
  x->scale = display_scale(x);
  int major, minor, count = 0;
  XRRMonitorInfo *native = NULL;
  if (x->randr_event >= 0 && XRRQueryVersion(x->display, &major, &minor) &&
      (major > 1 || (major == 1 && minor >= 5)))
    native = XRRGetMonitors(x->display, x->root, True, &count);
  XRRScreenResources *resources =
      native ? XRRGetScreenResourcesCurrent(x->display, x->root) : NULL;
  SWMonitor *monitors = NULL, **tail = &monitors;
  int total = native ? count : 1;
  for (int i = 0; i < total; ++i) {
    SWMonitor *monitor = calloc(1, sizeof(*monitor));
    if (!monitor) {
      sw_error(app, SWErrorNative, ENOMEM);
      app->error_pending = true;
      break;
    }
    monitor->native_id = native ? native[i].name : UINT64_MAX;
    monitor->data.key = monitor_key(app, monitor->native_id);
    monitor->data.scale = x->scale;
    monitor->data.position_known = 1;
    if (native) {
      XRRMonitorInfo *info = &native[i];
      monitor->data.primary = info->primary;
      monitor->data.x = info->x;
      monitor->data.y = info->y;
      monitor->data.width = monitor->data.pixel_width = info->width;
      monitor->data.height = monitor->data.pixel_height = info->height;
      monitor->data.refresh_millihertz = refresh_rate(x, resources, info);
      char *name = XGetAtomName(x->display, info->name);
      if (name) {
        monitor->data.name = (const uint8_t *)strdup(name);
        XFree(name);
      }
      if (monitor->data.name)
        monitor->data.name_length =
            (int64_t)strlen((const char *)monitor->data.name);
    } else {
      monitor->data.primary = 1;
      monitor->data.width = monitor->data.pixel_width =
          DisplayWidth(x->display, x->screen);
      monitor->data.height = monitor->data.pixel_height =
          DisplayHeight(x->display, x->screen);
    }
    *tail = monitor;
    tail = &monitor->next;
  }
  if (resources)
    XRRFreeScreenResources(resources);
  if (native)
    XRRFreeMonitors(native);
  sw_monitors_free(app->monitors);
  app->monitors = monitors;
  sw_monitor_changed(app);
  if (old_scale != x->scale) {
    for (SWWindow *window = app->windows; window; window = window->next) {
      window->snapshot.width = window->snapshot.pixel_width / x->scale;
      window->snapshot.height = window->snapshot.pixel_height / x->scale;
      window->snapshot.scale = x->scale;
      sw_geometry(window);
    }
  }
}

int64_t sw_x11_current_monitor(SWWindow *window) {
  double best_area = 0;
  int64_t key = 0;
  SWSnapshot *value = &window->snapshot;
  for (SWMonitor *monitor = window->app->monitors; monitor;
       monitor = monitor->next) {
    SWMonitorData *data = &monitor->data;
    double width = fmin(value->x + value->outer_width, data->x + data->width) -
                   fmax(value->x, data->x);
    double height =
        fmin(value->y + value->outer_height, data->y + data->height) -
        fmax(value->y, data->y);
    double area = fmax(0, width) * fmax(0, height);
    if (area > best_area) {
      best_area = area;
      key = data->key;
    }
  }
  return key;
}
