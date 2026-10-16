#ifndef SCOOP_PLATFORM_FILES_H
#define SCOOP_PLATFORM_FILES_H

#include <stdbool.h>
#include <stdint.h>

typedef struct {
    int32_t kind;
    int32_t permissions;
    int64_t size;
    int64_t accessed_seconds, accessed_nanos;
    int64_t modified_seconds, modified_nanos;
    int64_t changed_seconds, changed_nanos;
    int64_t created_seconds, created_nanos;
} ScoopPlatformFileMetadata;

int scoop_platform_file_open(const char *path, int flags, int mode);
int scoop_platform_file_sync(int fd, bool data_only);

#endif
