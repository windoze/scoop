#include "platform.h"

int64_t scoop_io_string_length(const ScoopIoChar *text) {
#ifdef _WIN32
    return (int64_t)wcslen(text);
#else
    return (int64_t)strlen(text);
#endif
}
