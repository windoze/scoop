#include "file.h"
#include <errno.h>
#include <string.h>

typedef struct {
    ScoopIoWork work;
    ScoopIoFile *file;
    char *path;
    int flags, mode;
} ScoopIoFileOpen;

static void open_execute(uv_work_t *native) {
    ScoopIoFileOpen *request = native->data;
    int fd = scoop_platform_file_open(request->path, request->flags, request->mode);
    int error = errno;
    request->file->fd = fd;
    ScoopIoEvent *event = &request->work.completion.event;
    if (fd < 0) {
        event->kind = SCOOP_IO_POSIX_ERROR;
        event->code = error;
    } else {
        event->kind = SCOOP_IO_RESOURCE;
        event->resource = request->file;
    }
}

static void open_finished(uv_work_t *native, int status) {
    ScoopIoFileOpen *request = native->data;
    free(request->path);
    if (request->file->fd < 0)
        free(request->file);
    scoop_io_work_finished(&request->work, status);
}

int scoop_io_file_open(ScoopIoLoop *loop, int64_t id, const char *path, int flags, int mode) {
    ScoopIoFileOpen *request = calloc(1, sizeof(*request));
    if (!request)
        return UV_ENOMEM;
    request->file = calloc(1, sizeof(*request->file));
    request->path = strdup(path);
    if (!request->file || !request->path) {
        free(request->file);
        free(request->path);
        free(request);
        return UV_ENOMEM;
    }
    request->file->fd = -1;
    request->file->loop = loop;
    request->flags = flags;
    request->mode = mode;
    int status = scoop_io_work_submit(&request->work, loop, id, open_execute, open_finished);
    if (status < 0) {
        free(request->file);
        free(request->path);
        free(request);
    }
    return status;
}
