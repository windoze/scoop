#ifndef SCOOP_PLATFORM_PROCESS_NATIVE_H
#define SCOOP_PLATFORM_PROCESS_NATIVE_H

#include <stdint.h>
#include "file_handles.h"

typedef struct {
    int32_t kind;
    ScoopPlatformFileHandle handle;
} ScoopPlatformStdio;

int scoop_platform_pipe_open(int32_t descriptors[2]);

#endif
