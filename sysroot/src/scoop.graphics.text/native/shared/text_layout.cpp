#include "text.h"

STTextLayout::STTextLayout(const STParagraph &source, double width, bool wrap,
                           const STInlineMetrics *inline_metrics, size_t count)
    : data(source.data), style(source.style) {
  auto builder =
      sgt::ParagraphBuilder::make(style, source.fonts, data->unicode);
  if (!builder)
    throw std::runtime_error("paragraph builder failed");
  size_t next_inline = 0;
  for (const auto &run : source.runs) {
    builder->pushStyle(run.style);
    if (run.placeholder) {
      if (next_inline >= count)
        throw std::invalid_argument("missing inline metrics");
      const auto &value = inline_metrics[next_inline++];
      builder->addPlaceholder(sgt::PlaceholderStyle(
          (float)value.width, (float)value.height,
          (sgt::PlaceholderAlignment)value.alignment,
          (sgt::TextBaseline)value.baseline, (float)value.offset));
    } else {
      builder->addText(run.text.data(), run.text.size());
    }
    builder->pop();
  }
  paragraph = builder->Build();
  if (!paragraph)
    throw std::runtime_error("paragraph creation failed");
  paragraph->layout(wrap ? (float)width
                         : std::numeric_limits<float>::infinity());
  if (!wrap)
    paragraph->layout(std::ceil(paragraph->getMaxIntrinsicWidth()));
  std::vector<sgt::LineMetrics> metrics;
  paragraph->getLineMetrics(metrics);
  for (size_t index = 0; index < metrics.size(); ++index) {
    const auto &line = metrics[index];
    // SkParagraph LineMetrics uses UTF-16; getActualTextRange uses UTF-8.
    auto actual = paragraph->getActualTextRange((int)index, true);
    size_t start = data->byte(line.fStartIndex);
    size_t end = data->byte(line.fEndIncludingNewline);
    size_t content = data->byte(line.fEndExcludingWhitespaces);
    size_t plain_end = data->byte(line.fEndIndex);
    lines.push_back(
        {{(int64_t)start, (int64_t)end},
         {(int64_t)start, (int64_t)content},
         {(int64_t)actual.start, (int64_t)actual.end},
         {line.fLeft, line.fBaseline - line.fAscent, line.fWidth, line.fHeight},
         line.fBaseline,
         line.fAscent,
         line.fDescent,
         end > plain_end ? 1 : 0});
  }
  info = {paragraph->getMaxWidth(),
          paragraph->getHeight(),
          paragraph->getMinIntrinsicWidth(),
          paragraph->getMaxIntrinsicWidth(),
          paragraph->getAlphabeticBaseline(),
          paragraph->getIdeographicBaseline(),
          (int64_t)lines.size(),
          (int64_t)data->graphemes.size(),
          paragraph->didExceedMaxLines() ? 1 : 0,
          data->text.empty() ? 0 : paragraph->unresolvedGlyphs()};
  if (lines.empty() && data->text.empty()) {
    double x = style.effective_align() == sgt::TextAlign::kRight ? info.width
               : style.effective_align() == sgt::TextAlign::kCenter
                   ? info.width / 2
                   : 0;
    lines.push_back({{0, 0},
                     {0, 0},
                     {0, 0},
                     {x, 0, 0, info.height},
                     info.alphabetic,
                     info.alphabetic,
                     info.height - info.alphabetic,
                     0});
    info.lines = 1;
  }
  if (!std::isfinite(info.width) || !std::isfinite(info.height))
    throw std::runtime_error("text layout overflow");
  place_inlines();
}

void STTextLayout::place_inlines() {
  // This SkParagraph version sorts placeholders by UTF-8 position within each
  // line. Only visible objects consume a box; zero-area boxes must still be
  // retained.
  const auto boxes = paragraph->getRectsForPlaceholders();
  size_t next = 0;
  for (const auto &range : data->inlines) {
    int index = paragraph->getLineNumberAt((size_t)range.start);
    if (index < 0 || range.start < lines[index].visible.start ||
        range.end > lines[index].visible.end) {
      inlines.push_back({{}, -1, 0, 0});
      continue;
    }
    if (next == boxes.size())
      throw std::runtime_error("missing visible placeholder box");
    const auto &box = boxes[next++];
    inlines.push_back({st_rect(box.rect), index,
                       box.direction == sgt::TextDirection::kLtr ? 1 : 0, 1});
  }
  if (next != boxes.size())
    throw std::runtime_error("unmatched placeholder box");
}

size_t STTextLayout::visible_end() const {
  return lines.empty() ? 0 : (size_t)lines.back().visible.end;
}
int STTextLayout::line_at(size_t byte, bool upstream) const {
  if (lines.empty() || (info.exceeded && byte > visible_end()))
    return -1;
  if (upstream && byte > 0)
    --byte;
  for (size_t i = 0; i < lines.size(); ++i) {
    if (byte >= (size_t)lines[i].range.start &&
        byte < (size_t)lines[i].range.end)
      return (int)i;
  }
  if (byte == data->text.size() || byte == visible_end())
    return (int)lines.size() - 1;
  return -1;
}

extern "C" {
void *scoop_graphics_text_layout(void *pointer, double width, bool wrap,
                                 const STInlineMetrics *metrics,
                                 int64_t count) {
  try {
    return new STTextLayout(*static_cast<STParagraph *>(pointer), width, wrap,
                            metrics, (size_t)count);
  } catch (...) {
    return nullptr;
  }
}
STLayoutInfo scoop_graphics_text_info(void *pointer) {
  return static_cast<STTextLayout *>(pointer)->info;
}
STTextLine scoop_graphics_text_line(void *pointer, int64_t index) {
  return static_cast<STTextLayout *>(pointer)->lines[(size_t)index];
}
STInlineBox scoop_graphics_text_inline_box(void *pointer, int64_t index) {
  return static_cast<STTextLayout *>(pointer)->inlines[(size_t)index];
}
void scoop_graphics_text_graphemes(void *pointer, int64_t *output) {
  const auto &values = static_cast<STTextLayout *>(pointer)->data->graphemes;
  std::copy(values.begin(), values.end(), output);
}
int32_t scoop_graphics_text_paint(void *pointer, void *target, STPoint origin) {
  try {
    static_cast<STTextLayout *>(pointer)->paragraph->paint(
        static_cast<SkCanvas *>(target), (float)origin.x, (float)origin.y);
    return 1;
  } catch (...) {
    return 0;
  }
}
}
