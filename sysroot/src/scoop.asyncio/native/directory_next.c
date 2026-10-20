#include "directory.h"
#include <errno.h>
#include <string.h>

typedef struct {
    ScoopIoWork work;
    ScoopIoDirectory *directory;
} DirectoryNext;

static void execute_next(uv_work_t *native) {
    DirectoryNext *request = native->data;
    const char *name = scoop_platform_directory_next(request->directory->handle);
    int saved_errno = errno;
    ScoopIoEvent *event = &request->work.completion.event;
    if (!name) {
        event->kind = saved_errno ? SCOOP_IO_POSIX_ERROR : SCOOP_IO_EOF;
        event->code = saved_errno;
        return;
    }
    char *copy = strdup(name);
    if (!copy) {
        event->kind = SCOOP_IO_UV_ERROR;
        event->code = UV_ENOMEM;
        return;
    }
    event->kind = SCOOP_IO_DIRECTORY_NAME;
    event->resource = copy;
}

static void finish_next(uv_work_t *native, int status) {
    DirectoryNext *request = native->data;
    scoop_io_work_finished(&request->work, status);
}

int scoop_io_directory_next(ScoopIoDirectory *directory, int64_t id) {
    DirectoryNext *request = calloc(1, sizeof(*request));
    if (!request)
        return UV_ENOMEM;
    request->directory = directory;
    int status = scoop_io_work_submit(&request->work, directory->loop, id, execute_next, finish_next);
    if (status < 0)
        free(request);
    return status;
}

void scoop_io_directory_name_free(char *name) { free(name); }
