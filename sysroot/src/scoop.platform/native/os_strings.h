#ifndef SCOOP_PLATFORM_OS_STRINGS_H
#define SCOOP_PLATFORM_OS_STRINGS_H

#ifdef _WIN32
#include "windows_api.h"
#endif
#include <stdint.h>

/* Owned malloc storage: UTF-16 units on Windows, bytes on POSIX. A successful
   optional query uses a null pointer only for absence, not for an empty string. */
typedef struct {
    void *data;
    int64_t length;
    int32_t error;
} ScoopPlatformString;

#endif
