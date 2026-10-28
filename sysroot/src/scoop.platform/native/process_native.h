#ifndef SCOOP_PLATFORM_PROCESS_NATIVE_H
#define SCOOP_PLATFORM_PROCESS_NATIVE_H

#include <stdint.h>
#include "file_handles.h"

typedef struct {
    int32_t kind;
    ScoopPlatformFileHandle handle;
} ScoopPlatformStdio;

typedef struct {
    uint64_t value;
} ScoopPlatformProcess;

typedef struct {
    ScoopPlatformProcess process;
    int32_t error;
} ScoopPlatformSpawnResult;

typedef struct {
    int64_t status;
    int32_t error;
} ScoopPlatformProcessWait;

static inline ScoopPlatformSpawnResult scoop_process_failure(int32_t error) {
    return (ScoopPlatformSpawnResult){{UINT64_MAX}, error};
}

int scoop_platform_pipe_open(int32_t descriptors[2]);

#endif
