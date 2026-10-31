#include "../shared/text.h"
#include "include/ports/SkTypeface_win.h"

sk_sp<SkFontMgr> sg_system_fonts() { return SkFontMgr_New_DirectWrite(); }
