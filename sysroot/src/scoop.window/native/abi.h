#ifndef SCOOP_WINDOW_ABI_H
#define SCOOP_WINDOW_ABI_H

#include <stdint.h>

typedef int32_t (*SWCallback)(void *, int64_t, int32_t);
typedef struct {
  int64_t window;
  int32_t kind, flags;
  int64_t a, b, c, d;
  double x, y, z, w, scale, dx, dy;
  const uint8_t *text;
  int64_t text_length;
} SWEventData;
typedef struct {
  int32_t kind;
  int64_t anchor, cursor;
  const uint8_t *text;
  int64_t text_length;
} SWTextEditData;
typedef struct {
  double width, height, pixel_width, pixel_height, scale;
  double x, y, outer_width, outer_height;
  int32_t visible, occluded, focused, minimized, maximized, dark;
  int32_t configured, position_known, redraw_ready;
  int64_t generation;
} SWSnapshot;
typedef struct {
  int64_t key, primary, refresh_millihertz;
  double x, y, width, height, pixel_width, pixel_height, scale;
  const uint8_t *name;
  int64_t name_length;
  int32_t position_known;
} SWMonitorData;

enum { SWApplied = 0, SWRequested = 1, SWUnsupported = 2, SWFailed = -1 };
enum {
  SWClose = 1,
  SWGeometry = 2,
  SWMoved = 3,
  SWFocused = 4,
  SWOccluded = 5,
  SWTheme = 6,
  SWRedraw = 7,
  SWKeyboard = 10,
  SWModifiers = 11,
  SWPointer = 12,
  SWTextBatch = 13,
  SWImeState = 14,
  SWFileDrop = 15,
  SWMonitors = 100,
  SWExit = 101
};

#endif
