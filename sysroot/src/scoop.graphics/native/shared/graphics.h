#pragma once

#include "include/core/SkCanvas.h"
#include "include/core/SkColorSpace.h"
#include "include/core/SkContourMeasure.h"
#include "include/core/SkData.h"
#include "include/core/SkImage.h"
#include "include/core/SkPathBuilder.h"
#include "include/core/SkSurface.h"
#include "include/gpu/ganesh/GrDirectContext.h"
#include <cmath>
#include <cstdint>
#include <memory>
#include <vector>

struct SGPoint {
  double x, y;
};
struct SGRect {
  double x, y, width, height;
};
struct SGColor {
  double red, green, blue, alpha;
};
struct SGTransform {
  double a, b, c, d, tx, ty;
};
struct SGSample {
  int32_t found;
  SGPoint position, tangent;
};

struct SGObject {
  virtual ~SGObject() = default;
};
struct SGPath : SGObject {
  SkPath path;
  explicit SGPath(SkPath p) : path(std::move(p)) {}
};
struct SGBuilder : SGObject {
  SkPathBuilder path;
};
struct SGMeasure : SGObject {
  std::vector<sk_sp<SkContourMeasure>> contours;
  double length = 0;
};
struct SGPaint : SGObject {
  SkPaint paint;
  std::vector<SkColor4f> colors;
  std::vector<float> positions;
};
struct SGImage : SGObject {
  sk_sp<SkImage> image;
  explicit SGImage(sk_sp<SkImage> i) : image(std::move(i)) {}
};
struct SGContext : SGObject {
  sk_sp<GrDirectContext> context;
};
struct SGTarget : SGObject {
  sk_sp<SkSurface> surface;
  sk_sp<GrDirectContext> context;
};
struct SGCanvas : SGObject {
  sk_sp<SkSurface> surface;
  SkCanvas *canvas;
  float scale;
  int base;
  std::vector<std::vector<SkPath>> clips;
  SGCanvas(sk_sp<SkSurface> s, float dpi)
      : surface(std::move(s)), canvas(surface->getCanvas()), scale(dpi),
        base(canvas->getSaveCount()), clips(1) {
    canvas->save();
    canvas->scale(dpi, dpi);
  }
  explicit SGCanvas(const SGCanvas &parent)
      : surface(parent.surface), canvas(parent.canvas), scale(parent.scale),
        base(canvas->getSaveCount()), clips(1, parent.clips.back()) {
    canvas->save();
  }
  ~SGCanvas() override { canvas->restoreToCount(base); }
  SkMatrix logical() const {
    return SkMatrix::Concat(SkMatrix::Scale(1 / scale, 1 / scale),
                            canvas->getLocalToDeviceAs3x3());
  }
};
struct SGGeometry : SGObject {
  SkMatrix matrix, inverse;
  bool invertible;
  std::vector<SkPath> clips;
  SkPath shape;
};

inline SkPoint sg_point(SGPoint p) { return {(float)p.x, (float)p.y}; }
inline SGPoint sg_point(SkPoint p) { return {p.x(), p.y()}; }
inline SkRect sg_rect(SGRect r) {
  return SkRect::MakeLTRB((float)r.x, (float)r.y, (float)(r.x + r.width),
                          (float)(r.y + r.height));
}
inline SGRect sg_rect(SkRect r) {
  return {r.x(), r.y(), r.width(), r.height()};
}
inline SkColor4f sg_color(SGColor c) {
  return {(float)c.red, (float)c.green, (float)c.blue, (float)c.alpha};
}
inline SkMatrix sg_matrix(SGTransform m) {
  return SkMatrix::MakeAll((float)m.a, (float)m.c, (float)m.tx, (float)m.b,
                           (float)m.d, (float)m.ty, 0, 0, 1);
}
inline SkImageInfo sg_info(int width, int height,
                           SkAlphaType alpha = kPremul_SkAlphaType) {
  return SkImageInfo::Make(width, height, kRGBA_8888_SkColorType, alpha,
                           SkColorSpace::MakeSRGB());
}
