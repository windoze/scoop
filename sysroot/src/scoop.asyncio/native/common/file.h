#ifndef SCOOP_ASYNCIO_FILE_H
#define SCOOP_ASYNCIO_FILE_H

#include "../../../scoop.platform/native/file_handles.h"
#include "platform.h"
#include "work.h"

/* Preallocated close work needs no new native allocation after open. */
typedef struct {
    ScoopIoWork closing;
    ScoopIoLoop *loop;
    ScoopPlatformFileHandle handle;
#ifdef _WIN32
    int64_t position;
    bool overlapped;
#endif
} ScoopIoFile;

static inline ScoopIoFile *scoop_io_file_new(ScoopIoLoop *loop) {
    ScoopIoFile *file = calloc(1, sizeof(*file));
    if (file) {
        file->loop = loop;
        file->handle.value = UINT64_MAX;
    }
    return file;
}

int scoop_io_file_open(ScoopIoLoop *loop, int64_t id, const ScoopIoChar *path, int flags, int mode);
int scoop_io_file_transfer(ScoopIoFile *file, int64_t id, void *bytes, int64_t count, bool writing);
int scoop_io_file_sync(ScoopIoFile *file, int64_t id, bool data_only);
int scoop_io_file_close(ScoopIoFile *file, int64_t id);
int scoop_io_file_seek(ScoopIoFile *file, int64_t id, int64_t offset, bool current);
int scoop_io_file_truncate(ScoopIoFile *file, int64_t id, int64_t size);
int scoop_io_file_metadata(ScoopIoFile *file, int64_t id);
int scoop_io_path_metadata(ScoopIoLoop *loop, int64_t id, const ScoopIoChar *path, bool follow);
ScoopPlatformFileMetadata scoop_io_metadata_take(ScoopPlatformFileMetadata *value);

#ifdef _WIN32
int scoop_io_file_associate(ScoopIoFile *file);
int scoop_io_file_transfer_work(ScoopIoFile *file, int64_t id, void *bytes, int64_t count,
                                bool writing);
#endif

#endif
