#ifndef SCOOP_PLATFORM_DIRECTORY_OWNER_H
#define SCOOP_PLATFORM_DIRECTORY_OWNER_H

#include "file_handles.h"

typedef struct {
    void *owner;
    int32_t error;
} ScoopPlatformDirectoryOpened;

typedef struct {
    const void *name;
    int64_t length;
    int32_t error;
} ScoopPlatformDirectoryEntry;

ScoopPlatformDirectoryOpened scoop_platform_directory_owner_open(const void *path);
ScoopPlatformDirectoryEntry scoop_platform_directory_owner_next(void *owner);
ScoopPlatformFileResult scoop_platform_directory_owner_close(void *owner);

#endif
