#include "directory.h"
#include <errno.h>
#include <string.h>

typedef struct {
    ScoopIoWork work;
    ScoopIoDirectory *directory;
    ScoopIoChar *path;
} DirectoryOpen;

static void execute_open(ScoopIoWork *native) {
    DirectoryOpen *request = (void *)native;
    ScoopPlatformDirectoryOpened opened = scoop_platform_directory_owner_open(request->path);
    request->directory->handle = opened.owner;
    ScoopIoEvent *event = &request->work.completion.event;
    if (opened.error)
        scoop_io_platform_error(event, opened.error);
    else {
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

int scoop_io_directory_open(ScoopIoLoop *loop, int64_t id, const ScoopIoChar *path) {
    DirectoryOpen *request = calloc(1, sizeof(*request));
    if (!request)
        return -ENOMEM;
    request->directory = calloc(1, sizeof(*request->directory));
    request->path = scoop_io_string_copy(path);
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
    ScoopPlatformFileResult result = scoop_platform_directory_owner_close(directory->handle);
    directory->handle = NULL;
    scoop_io_platform_error(&directory->closing.completion.event, result.error);
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
