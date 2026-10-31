#include "text.h"

SGTextLayout::SGTextLayout(const SGParagraph &source, double width, bool wrap)
    : data(source.data), style(source.style) {
  auto builder =
      sgt::ParagraphBuilder::make(style, source.fonts, data->unicode);
  if (!builder)
    throw std::runtime_error("paragraph builder failed");
  for (const auto &run : source.runs) {
    builder->pushStyle(run.style);
    builder->addText(run.text.data(), run.text.size());
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
}

size_t SGTextLayout::visible_end() const {
  return lines.empty() ? 0 : (size_t)lines.back().visible.end;
}
int SGTextLayout::line_at(size_t byte, bool upstream) const {
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
void *scoop_graphics_text_layout(void *pointer, double width, bool wrap) {
  try {
    return new SGTextLayout(*static_cast<SGParagraph *>(pointer), width, wrap);
  } catch (...) {
    return nullptr;
  }
}
SGLayoutInfo scoop_graphics_text_info(void *pointer) {
  return static_cast<SGTextLayout *>(pointer)->info;
}
SGTextLine scoop_graphics_text_line(void *pointer, int64_t index) {
  return static_cast<SGTextLayout *>(pointer)->lines[(size_t)index];
}
void scoop_graphics_text_graphemes(void *pointer, int64_t *output) {
  const auto &values = static_cast<SGTextLayout *>(pointer)->data->graphemes;
  std::copy(values.begin(), values.end(), output);
}
int32_t scoop_graphics_text_paint(void *pointer, void *target, SGPoint origin) {
  try {
    static_cast<SGTextLayout *>(pointer)->paragraph->paint(
        static_cast<SGCanvas *>(target)->canvas, (float)origin.x,
        (float)origin.y);
    return 1;
  } catch (...) {
    return 0;
  }
}
}
