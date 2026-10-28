#ifndef SCOOP_PLATFORM_FILE_HANDLES_H
#define SCOOP_PLATFORM_FILE_HANDLES_H

#ifdef _WIN32
#include "windows_api.h"
#endif
#include "files.h"

/* File handles, sockets and CRT descriptors have separate ownership and types. */
typedef struct {
    uint64_t value;
} ScoopPlatformFileHandle;
typedef struct {
    ScoopPlatformFileHandle handle;
    int32_t error;
} ScoopPlatformFileOpened;
typedef struct {
    int64_t value;
    int32_t error;
} ScoopPlatformFileResult;
typedef struct {
    ScoopPlatformFileMetadata data;
    int32_t error;
} ScoopPlatformFileMetadataResult;
typedef struct {
    ScoopPlatformFileHandle source, sink;
    int32_t error;
} ScoopPlatformPipeOpened;

#ifdef _WIN32
static inline HANDLE scoop_file_handle(ScoopPlatformFileHandle handle) {
    return (HANDLE)(uintptr_t)handle.value;
}
static inline ScoopPlatformFileHandle scoop_file_wrap(HANDLE handle) {
    return (ScoopPlatformFileHandle){(uint64_t)(uintptr_t)handle};
}
#else
static inline int scoop_file_handle(ScoopPlatformFileHandle handle) {
    return handle.value == UINT64_MAX ? -1 : (int)handle.value;
}
static inline ScoopPlatformFileHandle scoop_file_wrap(int descriptor) {
    return (ScoopPlatformFileHandle){descriptor < 0 ? UINT64_MAX : (uint64_t)descriptor};
}
#endif

ScoopPlatformFileOpened scoop_platform_handle_open(const void *path, int32_t flags, int32_t mode);
#ifdef _WIN32
ScoopPlatformFileOpened scoop_platform_handle_open_overlapped(const void *path, int32_t flags,
                                                              int32_t mode);
#endif
ScoopPlatformFileOpened scoop_platform_handle_duplicate(ScoopPlatformFileHandle handle);
ScoopPlatformFileOpened scoop_platform_handle_standard(int32_t stream);
ScoopPlatformFileResult scoop_platform_handle_close(ScoopPlatformFileHandle handle);
ScoopPlatformFileResult scoop_platform_handle_read(ScoopPlatformFileHandle handle, void *data,
                                                   int64_t count);
ScoopPlatformFileResult scoop_platform_handle_write(ScoopPlatformFileHandle handle,
                                                    const void *data, int64_t count);
ScoopPlatformFileResult scoop_platform_handle_seek(ScoopPlatformFileHandle handle, int64_t offset,
                                                   bool current);
ScoopPlatformFileResult scoop_platform_handle_truncate(ScoopPlatformFileHandle handle,
                                                       int64_t size);
ScoopPlatformFileResult scoop_platform_handle_sync(ScoopPlatformFileHandle handle, bool data_only);
ScoopPlatformFileResult scoop_platform_handle_same(ScoopPlatformFileHandle first,
                                                   ScoopPlatformFileHandle second);
ScoopPlatformFileResult scoop_platform_handle_lock(ScoopPlatformFileHandle handle,
                                                   int32_t operation);
ScoopPlatformFileMetadataResult scoop_platform_handle_metadata(ScoopPlatformFileHandle handle);
ScoopPlatformFileMetadataResult scoop_platform_path_metadata_result(const void *path, bool follow);
ScoopPlatformPipeOpened scoop_platform_handle_pipe(void);

#endif
