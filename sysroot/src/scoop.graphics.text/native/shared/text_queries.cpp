#include "text.h"

extern "C" {
STTextHit scoop_graphics_text_hit(void *pointer, STPoint point) {
  try {
    auto layout = static_cast<STTextLayout *>(pointer);
    auto &data = *layout->data;
    for (size_t i = 0; i < layout->inlines.size(); ++i) {
      const auto &box = layout->inlines[i];
      const auto &rect = box.rect;
      if (box.visible && point.x >= rect.x && point.x < rect.x + rect.width &&
          point.y >= rect.y && point.y < rect.y + rect.height) {
        bool trailing =
            (point.x >= rect.x + rect.width / 2) == (box.direction == 1);
        const auto &range = data.inlines[i];
        return {trailing ? range.end : range.start, trailing ? 0 : 1, 1};
      }
    }
    auto hit = layout->paragraph->getGlyphPositionAtCoordinate((float)point.x,
                                                               (float)point.y);
    size_t byte =
        data.byte(std::min((size_t)hit.position, data.to8.size() - 1));
    bool upstream = hit.affinity == sgt::Affinity::kUpstream;
    if (byte > layout->visible_end() && layout->info.exceeded) {
      byte = layout->visible_end();
      upstream = true;
    }
    if (!data.grapheme((int64_t)byte)) {
      auto end = std::upper_bound(data.graphemes.begin(), data.graphemes.end(),
                                  (int64_t)byte);
      int64_t start = *(end - 1);
      auto boxes = layout->paragraph->getRectsForRange(
          (unsigned)data.to16[start], (unsigned)data.to16[*end],
          sgt::RectHeightStyle::kMax, sgt::RectWidthStyle::kTight);
      bool trailing = upstream;
      if (!boxes.empty())
        trailing = (point.x >= boxes[0].rect.centerX()) ==
                   (boxes[0].direction == sgt::TextDirection::kLtr);
      byte = trailing ? (size_t)*end : (size_t)start;
      upstream = trailing;
    }
    return {(int64_t)byte, upstream ? 0 : 1, 1};
  } catch (...) {
    return {0, 1, 0};
  }
}
STCaret scoop_graphics_text_caret(void *pointer, int64_t byte, int32_t affinity,
                                  double thickness) {
  try {
    auto layout = static_cast<STTextLayout *>(pointer);
    auto &data = *layout->data;
    if (!data.scalar(byte) || !data.grapheme(byte))
      return {{}, -1};
    bool upstream = affinity == 0;
    int line_index = layout->line_at((size_t)byte, upstream);
    if (line_index < 0)
      return {{}, 0};
    const auto &line = layout->lines[line_index];
    double x = line.rect.x;
    auto edge =
        std::lower_bound(data.graphemes.begin(), data.graphemes.end(), byte);
    bool trailing = upstream && edge != data.graphemes.begin();
    if (byte == (int64_t)data.text.size() ||
        (layout->info.exceeded && byte == (int64_t)layout->visible_end()))
      trailing = edge != data.graphemes.begin();
    int64_t start = trailing ? *(edge - 1) : byte;
    int64_t end = trailing
                      ? byte
                      : (edge + 1 != data.graphemes.end() ? *(edge + 1) : byte);
    auto object =
        std::lower_bound(data.inlines.begin(), data.inlines.end(), start,
                         [](const STRange &range, int64_t position) {
                           return range.start < position;
                         });
    if (object != data.inlines.end() && object->start == start &&
        object->end == end) {
      const auto &box = layout->inlines[object - data.inlines.begin()];
      if (!box.visible)
        return {{}, 0};
      bool right = trailing == (box.direction == 1);
      x = box.rect.x + (right ? box.rect.width : 0);
      return {{x, line.rect.y, thickness, line.rect.height}, 1};
    }
    auto boxes = layout->paragraph->getRectsForRange(
        (unsigned)data.to16[start], (unsigned)data.to16[end],
        sgt::RectHeightStyle::kMax, sgt::RectWidthStyle::kTight);
    bool found = false;
    for (const auto &box : boxes) {
      if (box.rect.bottom() > line.rect.y &&
          box.rect.top() < line.rect.y + line.rect.height) {
        bool right = trailing == (box.direction == sgt::TextDirection::kLtr);
        x = right ? box.rect.right() : box.rect.left();
        found = true;
        break;
      }
    }
    if (!found && byte > line.range.start)
      x += layout->style.getTextDirection() == sgt::TextDirection::kLtr
               ? line.rect.width
               : 0;
    return {{x, line.rect.y, thickness, line.rect.height}, 1};
  } catch (...) {
    return {{}, -2};
  }
}
STSelection scoop_graphics_text_selection(void *pointer, int64_t start,
                                          int64_t end) {
  try {
    auto layout = static_cast<STTextLayout *>(pointer);
    if (start > end || !layout->data->scalar(start) ||
        !layout->data->scalar(end)) {
      return {nullptr, -1};
    }
    auto result = std::make_unique<STTextBoxes>();
    auto append_text = [&](int64_t from, int64_t to) {
      if (from == to)
        return;
      const auto boxes = layout->paragraph->getRectsForRange(
          (unsigned)layout->data->to16[from], (unsigned)layout->data->to16[to],
          sgt::RectHeightStyle::kMax, sgt::RectWidthStyle::kTight);
      result->values.insert(result->values.end(), boxes.begin(), boxes.end());
    };
    int64_t position = start;
    for (size_t i = 0; i < layout->data->inlines.size(); ++i) {
      const auto &range = layout->data->inlines[i];
      if (range.start < start)
        continue;
      if (range.end > end)
        break;
      append_text(position, range.start);
      const auto &box = layout->inlines[i];
      if (box.visible) {
        const auto &rect = box.rect;
        result->values.emplace_back(
            SkRect::MakeXYWH((float)rect.x, (float)rect.y, (float)rect.width,
                             (float)rect.height),
            box.direction == 1 ? sgt::TextDirection::kLtr
                               : sgt::TextDirection::kRtl);
      }
      position = range.end;
    }
    append_text(position, end);
    return {result.release(), 1};
  } catch (...) {
    return {nullptr, 0};
  }
}
int64_t scoop_graphics_text_boxes_count(void *pointer) {
  return (int64_t)static_cast<STTextBoxes *>(pointer)->values.size();
}
STTextBox scoop_graphics_text_box(void *pointer, int64_t index) {
  const auto &box = static_cast<STTextBoxes *>(pointer)->values[(size_t)index];
  return {st_rect(box.rect), box.direction == sgt::TextDirection::kLtr ? 1 : 0};
}
}
