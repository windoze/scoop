#include "graphics.h"

namespace {
struct NativeCanvasBorrow final : SGObject {
  SkCanvas *canvas;
  SkAutoCanvasRestore restore;
  explicit NativeCanvasBorrow(SkCanvas *value)
      : canvas(value), restore(value, true) {}
};
} // namespace

extern "C" {
void *scoop_graphics_canvas_borrow(SGCanvas *value) {
  try {
    return new NativeCanvasBorrow(value->canvas);
  } catch (...) {
    return nullptr;
  }
}
SkCanvas *scoop_graphics_canvas_pointer(void *borrow) {
  return static_cast<NativeCanvasBorrow *>(borrow)->canvas;
}
}
