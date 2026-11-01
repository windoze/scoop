#include "../shared/text.h"
#include "include/ports/SkFontMgr_mac_ct.h"

sk_sp<SkFontMgr> st_system_fonts() { return SkFontMgr_New_CoreText(nullptr); }
