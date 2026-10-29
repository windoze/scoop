#include "file.h"
#include <errno.h>
#include <string.h>

typedef struct {
    ScoopIoWork work;
    ScoopIoFile *file;
    ScoopIoChar *path;
    int flags, mode;
} ScoopIoFileOpen;

static void open_execute(ScoopIoWork *native) {
    ScoopIoFileOpen *request = (void *)native;
    ScoopIoFile *file = request->file;
    ScoopPlatformFileOpened opened;
#ifdef _WIN32
    file->overlapped = !(request->flags & 4);
    opened =
        file->overlapped
            ? scoop_platform_handle_open_overlapped(request->path, request->flags, request->mode)
            : scoop_platform_handle_open(request->path, request->flags, request->mode);
#else
    opened = scoop_platform_handle_open(request->path, request->flags, request->mode);
#endif
    file->handle = opened.handle;
    int error = opened.error;
#ifdef _WIN32
    if (!error && file->overlapped) {
        error = scoop_io_file_associate(file);
        if (error) {
            (void)scoop_platform_handle_close(file->handle);
            file->handle.value = UINT64_MAX;
        }
    }
#endif
    ScoopIoEvent *event = &request->work.completion.event;
    if (error)
        scoop_io_platform_error(event, error);
    else {
        event->kind = SCOOP_IO_RESOURCE;
        event->resource = request->file;
    }
}

static void open_finished(ScoopIoWork *native, int status) {
    ScoopIoFileOpen *request = (void *)native;
    free(request->path);
    if (request->file->handle.value == UINT64_MAX)
        free(request->file);
    scoop_io_work_finished(&request->work, status);
}

int scoop_io_file_open(ScoopIoLoop *loop, int64_t id, const ScoopIoChar *path, int flags,
                       int mode) {
    ScoopIoFileOpen *request = calloc(1, sizeof(*request));
    if (!request)
        return -ENOMEM;
    request->file = scoop_io_file_new(loop);
    request->path = scoop_io_string_copy(path);
    if (!request->file || !request->path) {
        free(request->file);
        free(request->path);
        free(request);
        return -ENOMEM;
    }
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
