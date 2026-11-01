#include "include/core/SkCanvas.h"
#include "include/core/SkPaint.h"

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
