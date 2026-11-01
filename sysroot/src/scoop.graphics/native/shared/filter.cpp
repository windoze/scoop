#include "graphics.h"
#include "include/effects/SkImageFilters.h"

extern "C" {
SGFilter *scoop_graphics_filter_blur(double x, double y) {
  try {
    // Skia can represent the valid zero-sigma identity by a null filter.
    return new SGFilter(
        SkImageFilters::Blur((float)x, (float)y, SkTileMode::kDecal, nullptr));
  } catch (...) {
    return nullptr;
  }
}
SGFilter *scoop_graphics_filter_shadow(SGPoint offset, double x, double y,
                                       SGColor color, bool only) {
  try {
    auto space = SkColorSpace::MakeSRGB();
    auto filter =
        only ? SkImageFilters::DropShadowOnly((float)offset.x, (float)offset.y,
                                              (float)x, (float)y,
                                              sg_color(color), space, nullptr)
             : SkImageFilters::DropShadow((float)offset.x, (float)offset.y,
                                          (float)x, (float)y, sg_color(color),
                                          space, nullptr);
    return new SGFilter(std::move(filter));
  } catch (...) {
    return nullptr;
  }
}
SGFilter *scoop_graphics_filter_then(SGFilter *first, SGFilter *next) {
  try {
    return new SGFilter(SkImageFilters::Compose(next->filter, first->filter));
  } catch (...) {
    return nullptr;
  }
}
}
