#pragma once

#include "include/core/SkCanvas.h"
#include "include/core/SkData.h"
#include "include/core/SkFontArguments.h"
#include "include/core/SkTypeface.h"
#include "modules/skparagraph/include/ParagraphBuilder.h"
#include "modules/skparagraph/include/TypefaceFontProvider.h"
#include "modules/skunicode/include/SkUnicode_icu.h"
#include "src/core/SkUTF.h"
#include <algorithm>
#include <cmath>
#include <cstdint>
#include <limits>
#include <memory>
#include <stdexcept>
#include <string>
#include <vector>

namespace sgt = skia::textlayout;

struct STPoint {
  double x, y;
};
struct STRect {
  double x, y, width, height;
};
struct STColor {
  double red, green, blue, alpha;
};
struct STObject {
  virtual ~STObject() = default;
};
inline SkColor4f st_color(STColor value) {
  return {(float)value.red, (float)value.green, (float)value.blue,
          (float)value.alpha};
}
inline STRect st_rect(const SkRect &value) {
  return {value.x(), value.y(), value.width(), value.height()};
}

sk_sp<SkFontMgr> st_system_fonts();

struct STFontEntry {
  sk_sp<SkTypeface> face;
  SkString family;
};
struct STFonts : STObject {
  sk_sp<SkFontMgr> system;
  std::vector<STFontEntry> entries;
  bool fallback;
  STFonts(sk_sp<SkFontMgr> manager, bool enabled)
      : system(std::move(manager)), fallback(enabled) {}
  sk_sp<sgt::FontCollection> snapshot() const;
};
struct STTextStyleData {
  STColor color;
  double size, height, letter, word;
  int32_t weight, width, slant, has_height;
};
struct STTextStyle : STObject {
  sgt::TextStyle style;
  std::vector<SkString> families;
  std::vector<SkFontArguments::VariationPosition::Coordinate> variations;
};
struct STRichRun {
  std::string text;
  sgt::TextStyle style;
  bool placeholder = false;
};
struct STRange {
  int64_t start, end;
};
struct STParagraphBuilder : STObject {
  sk_sp<sgt::FontCollection> fonts;
  sgt::ParagraphStyle style;
  std::string text;
  std::vector<STRichRun> runs;
  std::vector<STRange> inlines;
  explicit STParagraphBuilder(sk_sp<sgt::FontCollection> source)
      : fonts(std::move(source)) {}
};
struct STTextData {
  std::string text;
  sk_sp<SkUnicode> unicode;
  std::vector<size_t> to16, to8;
  std::vector<int64_t> graphemes;
  std::vector<STRange> inlines;
  STTextData(std::string value, std::vector<STRange> objects);
  bool scalar(int64_t index) const {
    return index >= 0 && (size_t)index < to16.size() &&
           to16[index] != std::numeric_limits<size_t>::max();
  }
  bool grapheme(int64_t index) const {
    return std::binary_search(graphemes.begin(), graphemes.end(), index);
  }
  size_t byte(size_t utf16) const { return to8.at(utf16); }
};
struct STParagraph : STObject {
  sk_sp<sgt::FontCollection> fonts;
  sgt::ParagraphStyle style;
  std::vector<STRichRun> runs;
  std::shared_ptr<STTextData> data;
  explicit STParagraph(const STParagraphBuilder &builder)
      : fonts(builder.fonts), style(builder.style), runs(builder.runs),
        data(std::make_shared<STTextData>(builder.text, builder.inlines)) {}
};
struct STTextLine {
  STRange range, content, visible;
  STRect rect;
  double baseline, ascent, descent;
  int32_t hard_break;
};
struct STLayoutInfo {
  double width, height, minimum, maximum, alphabetic, ideographic;
  int64_t lines, graphemes;
  int32_t exceeded, unresolved;
};
struct STTextHit {
  int64_t byte;
  int32_t affinity, status;
};
struct STCaret {
  STRect rect;
  int32_t status;
};
struct STTextBox {
  STRect rect;
  int32_t direction;
};
struct STTextBoxes : STObject {
  std::vector<sgt::TextBox> values;
};
struct STSelection {
  void *pointer;
  int32_t status;
};
struct STInlineMetrics {
  double width, height, offset;
  int32_t alignment, baseline;
};
struct STInlineBox {
  STRect rect;
  int64_t line;
  int32_t direction, visible;
};
struct STTextLayout : STObject {
  std::unique_ptr<sgt::Paragraph> paragraph;
  std::shared_ptr<STTextData> data;
  sgt::ParagraphStyle style;
  std::vector<STTextLine> lines;
  std::vector<STInlineBox> inlines;
  STLayoutInfo info;
  STTextLayout(const STParagraph &source, double width, bool wrap,
               const STInlineMetrics *metrics, size_t count);
  void place_inlines();
  size_t visible_end() const;
  int line_at(size_t byte, bool upstream) const;
};
