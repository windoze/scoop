#include "directory.h"
#include <errno.h>
#include <string.h>

typedef struct {
    ScoopIoWork work;
    ScoopIoDirectory *directory;
} DirectoryNext;

static void execute_next(ScoopIoWork *native) {
    DirectoryNext *request = (void *)native;
    ScoopPlatformDirectoryEntry entry =
        scoop_platform_directory_owner_next(request->directory->handle);
    ScoopIoEvent *event = &request->work.completion.event;
    if (!entry.name) {
        event->kind = SCOOP_IO_EOF;
        scoop_io_platform_error(event, entry.error);
        return;
    }
    ScoopIoChar *copy = calloc((size_t)entry.length + 1, sizeof(*copy));
    if (!copy) {
        event->kind = SCOOP_IO_POSIX_ERROR;
        event->code = ENOMEM;
        return;
    }
    memcpy(copy, entry.name, (size_t)entry.length * sizeof(*copy));
    event->kind = SCOOP_IO_DIRECTORY_NAME;
    event->resource = copy;
}

static void finish_next(ScoopIoWork *native, int status) {
    DirectoryNext *request = (void *)native;
    scoop_io_work_finished(&request->work, status);
}

int scoop_io_directory_next(ScoopIoDirectory *directory, int64_t id) {
    DirectoryNext *request = calloc(1, sizeof(*request));
    if (!request)
        return -ENOMEM;
    request->directory = directory;
    int status =
        scoop_io_work_submit(&request->work, directory->loop, id, execute_next, finish_next);
    if (status < 0)
        free(request);
    return status;
}

void scoop_io_directory_name_free(ScoopIoChar *name) { free(name); }
