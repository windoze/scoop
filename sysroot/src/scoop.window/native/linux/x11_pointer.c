#include "x11.h"

typedef struct {
  int number;
  double increment, value;
  bool known;
} SWScrollAxis;
struct SWXDevice {
  int id;
  SWScrollAxis horizontal, vertical;
  SWXDevice *next;
};

void sw_x11_devices_free(SWX11 *x) {
  while (x->devices) {
    SWXDevice *device = x->devices;
    x->devices = device->next;
    free(device);
  }
}

void sw_x11_devices(SWX11 *x) {
  int count;
  XIDeviceInfo *devices = XIQueryDevice(x->display, XIAllDevices, &count);
  if (!devices)
    return;
  sw_x11_devices_free(x);
  for (int i = 0; i < count; ++i) {
    SWXDevice *device = calloc(1, sizeof(*device));
    if (!device) {
      sw_error(x->app, SWErrorNative, ENOMEM);
      x->app->error_pending = true;
      break;
    }
    device->id = devices[i].deviceid;
    device->horizontal.number = device->vertical.number = -1;
    for (int c = 0; c < devices[i].num_classes; ++c) {
      XIAnyClassInfo *info = devices[i].classes[c];
      if (info->type != XIScrollClass)
        continue;
      XIScrollClassInfo *scroll = (XIScrollClassInfo *)info;
      if (!isfinite(scroll->increment) || !scroll->increment)
        continue;
      SWScrollAxis *axis = scroll->scroll_type == XIScrollTypeHorizontal
                               ? &device->horizontal
                               : &device->vertical;
      if (axis->number >= 0 && !(scroll->flags & XIScrollFlagPreferred))
        continue;
      axis->number = scroll->number;
      axis->increment = scroll->increment;
    }
    for (int c = 0; c < devices[i].num_classes; ++c) {
      XIAnyClassInfo *info = devices[i].classes[c];
      if (info->type != XIValuatorClass)
        continue;
      XIValuatorClassInfo *valuator = (XIValuatorClassInfo *)info;
      SWScrollAxis *axis =
          valuator->number == device->horizontal.number ? &device->horizontal
          : valuator->number == device->vertical.number ? &device->vertical
                                                        : NULL;
      if (axis && isfinite(valuator->value)) {
        axis->value = valuator->value;
        axis->known = true;
      }
    }
    device->next = x->devices;
    x->devices = device;
  }
  XIFreeDeviceInfo(devices);
}

void sw_x11_pointer_setup(SWWindow *window) {
  SWX11 *x = window->app->x11;
  if (x->xi_opcode < 0)
    return;
  unsigned char mask[XIMaskLen(XI_LASTEVENT)] = {0};
  XISetMask(mask, XI_Motion);
  XISetMask(mask, XI_ButtonPress);
  XISetMask(mask, XI_ButtonRelease);
  XISetMask(mask, XI_Enter);
  XISetMask(mask, XI_Leave);
  XIEventMask selection = {XIAllMasterDevices, sizeof(mask), mask};
  XISelectEvents(x->display, window->x->id, &selection, 1);
}

static SWXDevice *device_for(SWX11 *x, int id) {
  for (SWXDevice *device = x->devices; device; device = device->next)
    if (device->id == id)
      return device;
  return NULL;
}
static double scroll(SWScrollAxis *axis, int number, double value) {
  if (axis->number != number || !isfinite(value))
    return 0;
  double delta = axis->known ? (axis->value - value) / axis->increment : 0;
  axis->known = true;
  axis->value = value;
  return isfinite(delta) ? delta : 0;
}

void sw_x11_pointer_event(SWX11 *x, XGenericEventCookie *cookie) {
  if (cookie->evtype == XI_HierarchyChanged ||
      cookie->evtype == XI_DeviceChanged) {
    sw_x11_devices(x);
    return;
  }
  if (cookie->evtype == XI_Enter || cookie->evtype == XI_Leave) {
    XIEnterEvent *event = cookie->data;
    SWWindow *window = sw_x11_find(x->app, event->event);
    if (window)
      sw_pointer(window, cookie->evtype == XI_Enter ? 0 : 1,
                 event->event_x / window->snapshot.scale,
                 event->event_y / window->snapshot.scale, x->keyboard.modifiers,
                 0, 0, 0, 0);
    return;
  }
  if (cookie->evtype != XI_Motion && cookie->evtype != XI_ButtonPress &&
      cookie->evtype != XI_ButtonRelease)
    return;
  XIDeviceEvent *event = cookie->data;
  SWWindow *window = sw_x11_find(x->app, event->event);
  if (!window)
    return;
  SWXDevice *device = device_for(x, event->sourceid);
  if (!device)
    device = device_for(x, event->deviceid);
  double px = event->event_x / window->snapshot.scale,
         py = event->event_y / window->snapshot.scale;
  if (cookie->evtype == XI_Motion) {
    sw_pointer(window, 2, px, py, x->keyboard.modifiers, 0, 0, 0, 0);
    if (!device)
      return;
    double dx = 0, dy = 0;
    const double *values = event->valuators.values;
    for (int i = 0; i < event->valuators.mask_len * 8; ++i) {
      if (!XIMaskIsSet(event->valuators.mask, i))
        continue;
      double value = *values++;
      dx += scroll(&device->horizontal, i, value);
      dy += scroll(&device->vertical, i, value);
    }
    if (dx || dy)
      sw_pointer(window, 5, px, py, x->keyboard.modifiers, 0, dx, dy, 0);
  } else {
    int button = event->detail;
    bool pressed = cookie->evtype == XI_ButtonPress;
    if (button >= 4 && button <= 7) {
      bool valuator = device && (button <= 5 ? device->vertical.number >= 0
                                             : device->horizontal.number >= 0);
      if (pressed && !valuator)
        sw_pointer(window, 5, px, py, x->keyboard.modifiers, 0,
                   button == 6   ? 1
                   : button == 7 ? -1
                                 : 0,
                   button == 4   ? 1
                   : button == 5 ? -1
                                 : 0,
                   0);
    } else
      sw_pointer(window, pressed ? 3 : 4, px, py, x->keyboard.modifiers,
                 button == 1   ? 0
                 : button == 3 ? 1
                 : button == 2 ? 2
                               : button,
                 0, 0, 0);
  }
}
