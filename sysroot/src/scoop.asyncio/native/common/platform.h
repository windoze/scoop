#ifndef SCOOP_ASYNCIO_COMMON_PLATFORM_H
#define SCOOP_ASYNCIO_COMMON_PLATFORM_H

#include "../loop.h"
#include <string.h>
#ifdef _WIN32
#include <wchar.h>
typedef wchar_t ScoopIoChar;
static inline ScoopIoChar *scoop_io_string_copy(const ScoopIoChar *text) { return _wcsdup(text); }
#else
typedef char ScoopIoChar;
static inline ScoopIoChar *scoop_io_string_copy(const ScoopIoChar *text) { return strdup(text); }
#endif

static inline void scoop_io_platform_error(ScoopIoEvent *event, int error) {
    if (error) {
#ifdef _WIN32
        event->kind = SCOOP_IO_WIN32_ERROR;
#else
        event->kind = SCOOP_IO_POSIX_ERROR;
#endif
        event->code = error;
    }
}

#endif
