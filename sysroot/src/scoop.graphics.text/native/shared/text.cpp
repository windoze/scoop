#include "text.h"

sk_sp<sgt::FontCollection> STFonts::snapshot() const {
  auto result = sk_make_sp<sgt::FontCollection>();
  auto assets = sk_make_sp<sgt::TypefaceFontProvider>();
  for (const auto &entry : entries)
    assets->registerTypeface(entry.face, entry.family);
  result->setAssetFontManager(assets);
  if (fallback)
    result->setDefaultFontManager(system);
  else
    result->disableFontFallback();
  return result;
}

STTextData::STTextData(std::string value, std::vector<STRange> objects)
    : text(std::move(value)), unicode(SkUnicodes::ICU::Make()),
      to16(text.size() + 1, std::numeric_limits<size_t>::max()),
      inlines(std::move(objects)) {
  if (!unicode)
    throw std::runtime_error("ICU initialization failed");
  const char *cursor = text.data(), *end = cursor + text.size();
  while (cursor < end) {
    size_t start = cursor - text.data();
    SkUnichar scalar = SkUTF::NextUTF8(&cursor, end);
    if (scalar < 0)
      throw std::invalid_argument("invalid UTF-8");
    to16[start] = to8.size();
    to8.push_back(start);
    if (scalar > 0xffff)
      to8.push_back(start);
  }
  to16[text.size()] = to8.size();
  to8.push_back(text.size());
  auto breaks = unicode->makeBreakIterator(SkUnicode::BreakType::kGraphemes);
  if (!breaks || !breaks->setText(text.data(), (int)text.size()))
    throw std::runtime_error("ICU grapheme analysis failed");
  for (int position = breaks->first(); !breaks->isDone();
       position = breaks->next())
    graphemes.push_back(position);
  if (graphemes.empty() || graphemes.front() != 0)
    graphemes.insert(graphemes.begin(), 0);
  if ((size_t)graphemes.back() != text.size())
    graphemes.push_back(text.size());
  for (const auto &object : inlines) {
    graphemes.push_back(object.start);
    graphemes.push_back(object.end);
  }
  std::sort(graphemes.begin(), graphemes.end());
  graphemes.erase(std::unique(graphemes.begin(), graphemes.end()),
                  graphemes.end());
}

extern "C" {
void scoop_graphics_text_drop(void *pointer) {
  delete static_cast<STObject *>(pointer);
}
void *scoop_graphics_text_fonts_create(bool fallback) {
  try {
    auto manager = st_system_fonts();
    return manager ? new STFonts(std::move(manager), fallback) : nullptr;
  } catch (...) {
    return nullptr;
  }
}
int32_t scoop_graphics_text_fonts_add(void *pointer, const void *bytes,
                                      int64_t length, const char *alias,
                                      int64_t alias_length, int32_t index) {
  try {
    auto fonts = static_cast<STFonts *>(pointer);
    auto data = SkData::MakeWithCopy(bytes, (size_t)length);
    auto face = fonts->system->makeFromData(std::move(data), index);
    if (!face)
      return -1;
    fonts->entries.push_back(
        {std::move(face), SkString(alias, (size_t)alias_length)});
    return 1;
  } catch (...) {
    return 0;
  }
}
void *scoop_graphics_text_style_create(STTextStyleData value) {
  try {
    auto result = std::make_unique<STTextStyle>();
    result->style.setColor(st_color(value.color).toSkColor());
    result->style.setFontSize((float)value.size);
    result->style.setFontStyle(SkFontStyle(value.weight, value.width,
                                           (SkFontStyle::Slant)value.slant));
    result->style.setLetterSpacing((float)value.letter);
    result->style.setWordSpacing((float)value.word);
    if (value.has_height) {
      result->style.setHeightOverride(true);
      result->style.setHeight((float)value.height);
    }
    return result.release();
  } catch (...) {
    return nullptr;
  }
}
int32_t scoop_graphics_text_style_string(void *pointer, const char *value,
                                         int64_t length, bool locale) {
  try {
    auto style = static_cast<STTextStyle *>(pointer);
    SkString text(value, (size_t)length);
    if (locale)
      style->style.setLocale(text);
    else {
      style->families.push_back(text);
      style->style.setFontFamilies(style->families);
    }
    return 1;
  } catch (...) {
    return 0;
  }
}
int32_t scoop_graphics_text_style_feature(void *pointer, const char *tag,
                                          double value, bool variation) {
  try {
    auto style = static_cast<STTextStyle *>(pointer);
    if (variation) {
      SkFourByteTag axis = SkSetFourByteTag(tag[0], tag[1], tag[2], tag[3]);
      style->variations.push_back({axis, (float)value});
      SkFontArguments args;
      args.setVariationDesignPosition(
          {style->variations.data(), (int)style->variations.size()});
      style->style.setFontArguments(args);
    } else {
      style->style.addFontFeature(SkString(tag, 4), (int)value);
    }
    return 1;
  } catch (...) {
    return 0;
  }
}
void *scoop_graphics_text_paragraph_begin(void *pointer, void *style_pointer,
                                          int32_t alignment, int32_t direction,
                                          int64_t max_lines,
                                          const char *ellipsis,
                                          int64_t length) {
  try {
    auto result = std::make_unique<STParagraphBuilder>(
        static_cast<STFonts *>(pointer)->snapshot());
    result->style.setTextStyle(
        static_cast<STTextStyle *>(style_pointer)->style);
    result->style.setTextAlign((sgt::TextAlign)alignment);
    result->style.setTextDirection((sgt::TextDirection)direction);
    result->style.setApplyRoundingHack(false);
    if (max_lines >= 0) {
      result->style.setMaxLines((size_t)max_lines);
      result->style.setEllipsis(SkString(ellipsis, (size_t)length));
    }
    return result.release();
  } catch (...) {
    return nullptr;
  }
}
int32_t scoop_graphics_text_paragraph_run(void *pointer, void *style_pointer,
                                          const char *text, int64_t length) {
  try {
    auto builder = static_cast<STParagraphBuilder *>(pointer);
    auto &style = static_cast<STTextStyle *>(style_pointer)->style;
    if ((uint64_t)length + builder->text.size() > INT32_MAX)
      return -1;
    builder->text.append(text, (size_t)length);
    builder->runs.push_back({std::string(text, (size_t)length), style});
    return 1;
  } catch (...) {
    return 0;
  }
}
void *scoop_graphics_text_paragraph_finish(void *pointer) {
  try {
    return new STParagraph(*static_cast<STParagraphBuilder *>(pointer));
  } catch (...) {
    return nullptr;
  }
}
int32_t scoop_graphics_text_paragraph_inline(void *pointer,
                                             void *style_pointer) {
  try {
    auto builder = static_cast<STParagraphBuilder *>(pointer);
    auto &style = static_cast<STTextStyle *>(style_pointer)->style;
    const auto start = (int64_t)builder->text.size();
    if (start > INT32_MAX - 3)
      return -1;
    builder->text.append("\xef\xbf\xbc", 3);
    builder->runs.push_back({{}, style, true});
    builder->inlines.push_back({start, start + 3});
    return 1;
  } catch (...) {
    return 0;
  }
}
}
