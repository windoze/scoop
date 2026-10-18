#ifndef SCOOP_ASYNCIO_FILE_H
#define SCOOP_ASYNCIO_FILE_H

#include "../../scoop.platform/native/files.h"
#include "../../scoop.platform/native/io.h"
#include "work.h"

/* Preallocated close work needs no new native allocation after open. */
typedef struct {
    ScoopIoWork closing;
    ScoopIoLoop *loop;
    int fd;
} ScoopIoFile;

int scoop_io_file_open(ScoopIoLoop *loop, int64_t id, const char *path, int flags, int mode);
int scoop_io_file_transfer(ScoopIoFile *file, int64_t id, void *bytes, int64_t count, bool writing);
int scoop_io_file_sync(ScoopIoFile *file, int64_t id, bool data_only);
int scoop_io_file_close(ScoopIoFile *file, int64_t id);

#endif
