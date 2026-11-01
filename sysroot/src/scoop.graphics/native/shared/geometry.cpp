#include "graphics.h"

static int32_t clip(SGCanvas *value, const SkPath &path) {
  try {
    auto transformed = path.tryMakeTransform(value->logical());
    if (!transformed)
      return -1;
    value->clips.back().push_back(std::move(*transformed));
    value->canvas->clipPath(path, true);
    return 1;
  } catch (...) {
    return 0;
  }
}
static SGGeometry *geometry(SGCanvas *value, SkPath shape) {
  try {
    auto result = std::make_unique<SGGeometry>();
    result->matrix = value->logical();
    result->invertible = result->matrix.invert(&result->inverse);
    result->clips = value->clips.back();
    result->shape = std::move(shape);
    return result.release();
  } catch (...) {
    return nullptr;
  }
}

extern "C" {
int32_t scoop_graphics_canvas_save(SGCanvas *value, double opacity, bool layer,
                                   SGFilter *filter, int32_t blend) {
  SkBlendMode mode;
  switch (blend) {
  case 0:
    mode = SkBlendMode::kSrcOver;
    break;
  case 1:
    mode = SkBlendMode::kSrc;
    break;
  case 2:
    mode = SkBlendMode::kDstIn;
    break;
  case 3:
    mode = SkBlendMode::kPlus;
    break;
  default:
    return -1;
  }
  try {
    value->clips.push_back(value->clips.back());
    if (layer) {
      SkPaint paint;
      paint.setAlphaf((float)opacity);
      paint.setBlendMode(mode);
      if (filter)
        paint.setImageFilter(filter->filter);
      value->canvas->saveLayer(nullptr, &paint);
    } else {
      value->canvas->save();
    }
    return 1;
  } catch (...) {
    return 0;
  }
}
int32_t scoop_graphics_canvas_restore(SGCanvas *value) {
  if (value->clips.size() <= 1)
    return -1;
  value->canvas->restore();
  value->clips.pop_back();
  return 1;
}
int32_t scoop_graphics_canvas_concat(SGCanvas *value, SGTransform transform) {
  SkMatrix matrix = sg_matrix(transform);
  SkMatrix next =
      SkMatrix::Concat(value->canvas->getLocalToDeviceAs3x3(), matrix);
  if (!next.isFinite())
    return -1;
  value->canvas->concat(matrix);
  return 1;
}
int32_t scoop_graphics_canvas_clip_rect(SGCanvas *value, SGRect rect) {
  try {
    return clip(value, SkPathBuilder().addRect(sg_rect(rect)).detach());
  } catch (...) {
    return 0;
  }
}
int32_t scoop_graphics_canvas_clip_path(SGCanvas *value, SGPath *path) {
  return clip(value, path->path);
}
SGGeometry *scoop_graphics_geometry_rect(SGCanvas *value, SGRect rect) {
  try {
    return geometry(value, SkPathBuilder().addRect(sg_rect(rect)).detach());
  } catch (...) {
    return nullptr;
  }
}
SGGeometry *scoop_graphics_geometry_path(SGCanvas *value, SGPath *path) {
  return geometry(value, path->path);
}
int32_t scoop_graphics_geometry_hit(SGGeometry *value, SGPoint point) {
  if (!value->invertible)
    return 0;
  SkPoint window = sg_point(point);
  for (const SkPath &clip : value->clips)
    if (!clip.contains(window))
      return 0;
  return value->shape.contains(value->inverse.mapPoint(window));
}
SGSample scoop_graphics_geometry_point(SGGeometry *value, SGPoint point,
                                       bool inverse) {
  if (inverse && !value->invertible)
    return {};
  SkPoint result =
      (inverse ? value->inverse : value->matrix).mapPoint(sg_point(point));
  if (!result.isFinite())
    return {};
  return {1, sg_point(result), {0, 0}};
}
SGRect scoop_graphics_geometry_rect_map(SGGeometry *value, SGRect rect) {
  return sg_rect(value->matrix.mapRect(sg_rect(rect)));
}
}
