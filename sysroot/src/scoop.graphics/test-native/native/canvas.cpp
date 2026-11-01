#include "include/core/SkCanvas.h"
#include "include/core/SkPaint.h"
#include <cmath>
#include <cstdint>
#include <limits>

extern "C" bool scoop_graphics_test_canvas_marker(SkCanvas *canvas) {
  try {
    canvas->save();
    canvas->translate(8, 4);
    canvas->clipRect(SkRect::MakeWH(6, 6));
    SkPaint paint;
    paint.setColor(SK_ColorRED);
    canvas->drawRect(SkRect::MakeWH(12, 12), paint);
    return true;
  } catch (...) {
    return false;
  }
}

extern "C" int64_t scoop_graphics_test_canvas_pixel(SkCanvas *canvas, double x,
                                                    double y) {
  const SkPoint point =
      canvas->getLocalToDeviceAs3x3().mapPoint({(float)x, (float)y});
  if (!std::isfinite(point.x()) || !std::isfinite(point.y()) || point.x() < 0 ||
      point.y() < 0 || (double)point.x() >= std::numeric_limits<int>::max() ||
      (double)point.y() >= std::numeric_limits<int>::max())
    return -1;
  uint8_t rgba[4] = {};
  if (!canvas->readPixels(
          SkImageInfo::Make(1, 1, kRGBA_8888_SkColorType, kPremul_SkAlphaType),
          rgba, 4, (int)point.x(), (int)point.y()))
    return -1;
  return (int64_t)rgba[0] | ((int64_t)rgba[1] << 8) | ((int64_t)rgba[2] << 16);
}
