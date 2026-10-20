#ifndef SCOOP_ASYNCIO_DIRECTORY_H
#define SCOOP_ASYNCIO_DIRECTORY_H

#include "../../scoop.platform/native/directory.h"
#include "work.h"

typedef struct {
    ScoopIoWork closing;
    ScoopIoLoop *loop;
    void *handle;
} ScoopIoDirectory;

int scoop_io_directory_open(ScoopIoLoop *loop, int64_t id, const char *path);
int scoop_io_directory_next(ScoopIoDirectory *directory, int64_t id);
int scoop_io_directory_close(ScoopIoDirectory *directory, int64_t id);
void scoop_io_directory_name_free(char *name);

#endif
