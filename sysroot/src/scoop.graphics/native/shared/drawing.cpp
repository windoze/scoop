#include "graphics.h"
#include "include/core/SkPixmap.h"
#include "include/core/SkRRect.h"
#include "include/effects/SkGradient.h"

extern "C" {
SGPaint *scoop_graphics_paint_create(SGColor color, int32_t style, double width,
                                     int32_t cap, int32_t join, double miter,
                                     bool aa) {
  try {
    auto result = std::make_unique<SGPaint>();
    result->paint.setColor4f(sg_color(color), SkColorSpace::MakeSRGB().get());
    result->paint.setStyle(style ? SkPaint::kStroke_Style
                                 : SkPaint::kFill_Style);
    result->paint.setStrokeWidth((float)width);
    result->paint.setStrokeCap((SkPaint::Cap)cap);
    result->paint.setStrokeJoin((SkPaint::Join)join);
    result->paint.setStrokeMiter((float)miter);
    result->paint.setAntiAlias(aa);
    return result.release();
  } catch (...) {
    return nullptr;
  }
}
int32_t scoop_graphics_paint_stop(SGPaint *value, double offset,
                                  SGColor color) {
  try {
    if (!value->positions.empty() && (float)offset <= value->positions.back())
      return -1;
    value->colors.push_back(sg_color(color));
    value->positions.push_back((float)offset);
    return 1;
  } catch (...) {
    return 0;
  }
}
int32_t scoop_graphics_paint_gradient(SGPaint *value, int32_t kind,
                                      SGPoint start, SGPoint end,
                                      double radius) {
  try {
    SkGradient gradient(SkGradient::Colors(value->colors, value->positions,
                                           SkTileMode::kClamp,
                                           SkColorSpace::MakeSRGB()),
                        {});
    SkPoint points[] = {sg_point(start), sg_point(end)};
    if ((kind == 0 && points[0] == points[1]) ||
        (kind == 1 && (float)radius <= 0))
      return -1;
    auto shader =
        kind == 0
            ? SkShaders::LinearGradient(points, gradient)
            : SkShaders::RadialGradient(points[0], (float)radius, gradient);
    if (!shader)
      return 0;
    value->paint.setShader(std::move(shader));
    return 1;
  } catch (...) {
    return 0;
  }
}
SGTarget *scoop_graphics_raster_create(int32_t width, int32_t height) {
  try {
    auto result = std::make_unique<SGTarget>();
    result->surface = SkSurfaces::Raster(sg_info(width, height));
    return result->surface ? result.release() : nullptr;
  } catch (...) {
    return nullptr;
  }
}
SGCanvas *scoop_graphics_target_canvas(SGTarget *target, double scale) {
  try {
    return new SGCanvas(target->surface, (float)scale);
  } catch (...) {
    return nullptr;
  }
}
int32_t scoop_graphics_target_submit(SGTarget *target, bool wait) {
  try {
    return !target->context ||
           target->context
               ->flushAndSubmit(target->surface.get(),
                                wait ? GrSyncCpu::kYes : GrSyncCpu::kNo)
               .fSuccess;
  } catch (...) {
    return 0;
  }
}
int32_t scoop_graphics_target_read(SGTarget *target, uint8_t *bytes) {
  try {
    if (!scoop_graphics_target_submit(target, true))
      return 0;
    auto info = sg_info(target->surface->width(), target->surface->height(),
                        kUnpremul_SkAlphaType);
    return target->surface->readPixels(info, bytes, info.minRowBytes(), 0, 0);
  } catch (...) {
    return 0;
  }
}
SGImage *scoop_graphics_target_snapshot(SGTarget *target) {
  try {
    auto result = target->surface->makeImageSnapshot();
    if (result && target->context) {
      if (!scoop_graphics_target_submit(target, true))
        return nullptr;
      result = result->makeRasterImage(target->context.get());
    }
    return result ? new SGImage(std::move(result)) : nullptr;
  } catch (...) {
    return nullptr;
  }
}
SGImage *scoop_graphics_image_rgba(int32_t width, int32_t height,
                                   const uint8_t *bytes) {
  try {
    auto info = sg_info(width, height, kUnpremul_SkAlphaType);
    auto result = SkImages::RasterFromPixmapCopy(
        SkPixmap(info, bytes, info.minRowBytes()));
    return result ? new SGImage(std::move(result)) : nullptr;
  } catch (...) {
    return nullptr;
  }
}
SGImage *scoop_graphics_image_decode(const uint8_t *bytes, int64_t count) {
  try {
    auto result = SkImages::DeferredFromEncodedData(
        SkData::MakeWithCopy(bytes, (size_t)count));
    if (result)
      result = result->makeRasterImage();
    return result ? new SGImage(std::move(result)) : nullptr;
  } catch (...) {
    return nullptr;
  }
}
int32_t scoop_graphics_image_dimension(SGImage *image, bool height) {
  return height ? image->image->height() : image->image->width();
}
int32_t scoop_graphics_canvas_clear(SGCanvas *value, SGColor color) {
  try {
    value->canvas->clear(sg_color(color));
    return 1;
  } catch (...) {
    return 0;
  }
}
int32_t scoop_graphics_canvas_shape(SGCanvas *value, int32_t kind, SGRect rect,
                                    double rx, double ry, SGPaint *paint) {
  try {
    switch (kind) {
    case 0:
      value->canvas->drawRect(sg_rect(rect), paint->paint);
      break;
    case 1:
      value->canvas->drawOval(sg_rect(rect), paint->paint);
      break;
    case 2:
      value->canvas->drawRoundRect(sg_rect(rect), (float)rx, (float)ry,
                                   paint->paint);
      break;
    default:
      return -1;
    }
    return 1;
  } catch (...) {
    return 0;
  }
}
int32_t scoop_graphics_canvas_line(SGCanvas *value, SGPoint start, SGPoint end,
                                   SGPaint *paint) {
  try {
    value->canvas->drawLine(sg_point(start), sg_point(end), paint->paint);
    return 1;
  } catch (...) {
    return 0;
  }
}
int32_t scoop_graphics_canvas_path(SGCanvas *value, SGPath *path,
                                   SGPaint *paint) {
  try {
    value->canvas->drawPath(path->path, paint->paint);
    return 1;
  } catch (...) {
    return 0;
  }
}
int32_t scoop_graphics_canvas_image(SGCanvas *value, SGImage *image,
                                    SGRect rect, int32_t sampling,
                                    double opacity) {
  try {
    SkPaint paint;
    paint.setAlphaf((float)opacity);
    value->canvas->drawImageRect(
        image->image, sg_rect(rect),
        SkSamplingOptions(sampling ? SkFilterMode::kLinear
                                   : SkFilterMode::kNearest),
        &paint);
    return 1;
  } catch (...) {
    return 0;
  }
}
}
