#include "directory.h"
#include <errno.h>
#include <string.h>

typedef struct {
    ScoopIoWork work;
    ScoopIoDirectory *directory;
    char *path;
} DirectoryOpen;

static void execute_open(ScoopIoWork *native) {
    DirectoryOpen *request = (void *)native;
    void *handle = scoop_platform_directory_open(request->path);
    int saved_errno = errno;
    request->directory->handle = handle;
    ScoopIoEvent *event = &request->work.completion.event;
    if (!handle) {
        event->kind = SCOOP_IO_POSIX_ERROR;
        event->code = saved_errno;
    } else {
        event->kind = SCOOP_IO_RESOURCE;
        event->resource = request->directory;
    }
}

static void finish_open(ScoopIoWork *native, int status) {
    DirectoryOpen *request = (void *)native;
    free(request->path);
    if (!request->directory->handle)
        free(request->directory);
    scoop_io_work_finished(&request->work, status);
}

int scoop_io_directory_open(ScoopIoLoop *loop, int64_t id, const char *path) {
    DirectoryOpen *request = calloc(1, sizeof(*request));
    if (!request)
        return -ENOMEM;
    request->directory = calloc(1, sizeof(*request->directory));
    request->path = strdup(path);
    if (!request->directory || !request->path) {
        free(request->directory);
        free(request->path);
        free(request);
        return -ENOMEM;
    }
    request->directory->loop = loop;
    int status = scoop_io_work_submit(&request->work, loop, id, execute_open, finish_open);
    if (status < 0) {
        free(request->directory);
        free(request->path);
        free(request);
    }
    return status;
}

static void execute_close(ScoopIoWork *native) {
    ScoopIoDirectory *directory = (void *)native;
    int result = scoop_platform_directory_close(directory->handle);
    int saved_errno = errno;
    directory->handle = NULL;
    if (result < 0) {
        directory->closing.completion.event.kind = SCOOP_IO_POSIX_ERROR;
        directory->closing.completion.event.code = saved_errno;
    }
}

static void finish_close(ScoopIoWork *native, int status) {
    ScoopIoDirectory *directory = (void *)native;
    scoop_io_work_finished(&directory->closing, status);
    /* The completion queue releases the allocation containing close work. */
}

int scoop_io_directory_close(ScoopIoDirectory *directory, int64_t id) {
    // Opening the directory started the workers; close uses its preallocated work.
    return scoop_io_work_submit(&directory->closing, directory->loop, id, execute_close,
                                finish_close);
}
