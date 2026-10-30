#pragma once

#include "graphics.h"
#include "include/core/SkFontArguments.h"
#include "include/core/SkTypeface.h"
#include "modules/skparagraph/include/ParagraphBuilder.h"
#include "modules/skparagraph/include/TypefaceFontProvider.h"
#include "modules/skunicode/include/SkUnicode_icu.h"
#include "src/core/SkUTF.h"
#include <algorithm>
#include <limits>
#include <stdexcept>
#include <string>

namespace sgt = skia::textlayout;

sk_sp<SkFontMgr> sg_system_fonts();

struct SGFontEntry {
  sk_sp<SkTypeface> face;
  SkString family;
};
struct SGFonts : SGObject {
  sk_sp<SkFontMgr> system;
  std::vector<SGFontEntry> entries;
  bool fallback;
  SGFonts(sk_sp<SkFontMgr> manager, bool enabled)
      : system(std::move(manager)), fallback(enabled) {}
  sk_sp<sgt::FontCollection> snapshot() const;
};
struct SGTextStyleData {
  SGColor color;
  double size, height, letter, word;
  int32_t weight, width, slant, has_height;
};
struct SGTextStyle : SGObject {
  sgt::TextStyle style;
  std::vector<SkString> families;
  std::vector<SkFontArguments::VariationPosition::Coordinate> variations;
};
struct SGRichRun {
  std::string text;
  sgt::TextStyle style;
};
struct SGParagraphBuilder : SGObject {
  sk_sp<sgt::FontCollection> fonts;
  sgt::ParagraphStyle style;
  std::string text;
  std::vector<SGRichRun> runs;
  explicit SGParagraphBuilder(sk_sp<sgt::FontCollection> source)
      : fonts(std::move(source)) {}
};
struct SGTextData {
  std::string text;
  sk_sp<SkUnicode> unicode;
  std::vector<size_t> to16, to8;
  std::vector<int64_t> graphemes;
  explicit SGTextData(std::string value);
  bool scalar(int64_t index) const {
    return index >= 0 && (size_t)index < to16.size() &&
           to16[index] != std::numeric_limits<size_t>::max();
  }
  bool grapheme(int64_t index) const {
    return std::binary_search(graphemes.begin(), graphemes.end(), index);
  }
  size_t byte(size_t utf16) const { return to8.at(utf16); }
};
struct SGParagraph : SGObject {
  sk_sp<sgt::FontCollection> fonts;
  sgt::ParagraphStyle style;
  std::vector<SGRichRun> runs;
  std::shared_ptr<SGTextData> data;
  explicit SGParagraph(const SGParagraphBuilder &builder)
      : fonts(builder.fonts), style(builder.style), runs(builder.runs),
        data(std::make_shared<SGTextData>(builder.text)) {}
};
struct SGRange {
  int64_t start, end;
};
struct SGTextLine {
  SGRange range, content, visible;
  SGRect rect;
  double baseline, ascent, descent;
  int32_t hard_break;
};
struct SGLayoutInfo {
  double width, height, minimum, maximum, alphabetic, ideographic;
  int64_t lines, graphemes;
  int32_t exceeded, unresolved;
};
struct SGTextHit {
  int64_t byte;
  int32_t affinity, status;
};
struct SGCaret {
  SGRect rect;
  int32_t status;
};
struct SGTextBox {
  SGRect rect;
  int32_t direction;
};
struct SGTextBoxes : SGObject {
  std::vector<sgt::TextBox> values;
};
struct SGSelection {
  void *pointer;
  int32_t status;
};
struct SGTextLayout : SGObject {
  std::unique_ptr<sgt::Paragraph> paragraph;
  std::shared_ptr<SGTextData> data;
  sgt::ParagraphStyle style;
  std::vector<SGTextLine> lines;
  SGLayoutInfo info;
  SGTextLayout(const SGParagraph &source, double width, bool wrap);
  size_t visible_end() const;
  int line_at(size_t byte, bool upstream) const;
};
