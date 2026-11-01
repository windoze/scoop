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
SGCanvas *scoop_graphics_canvas_scope(SGCanvas *value) {
  try {
    return new SGCanvas(*value);
  } catch (...) {
    return nullptr;
  }
}
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
