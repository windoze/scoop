#include "file.h"
#include <errno.h>
#include <string.h>

typedef struct {
    ScoopIoWork work;
    ScoopIoFile *file;
    char *path;
    bool follow;
} MetadataWork;

static void execute_metadata(ScoopIoWork *native) {
    MetadataWork *request = (void *)native;
    ScoopPlatformFileMetadata data = scoop_platform_file_metadata(
        request->file ? request->file->fd : -1, request->path, request->follow);
    int saved_errno = errno;
    ScoopIoEvent *event = &request->work.completion.event;
    if (data.kind < 0) {
        event->kind = SCOOP_IO_POSIX_ERROR;
        event->code = saved_errno;
        return;
    }
    ScoopPlatformFileMetadata *result = malloc(sizeof(*result));
    if (!result) {
        event->kind = SCOOP_IO_POSIX_ERROR;
        event->code = ENOMEM;
        return;
    }
    *result = data;
    event->kind = SCOOP_IO_METADATA;
    event->resource = result;
}

static void finish_metadata(ScoopIoWork *native, int status) {
    MetadataWork *request = (void *)native;
    free(request->path);
    scoop_io_work_finished(&request->work, status);
}

static int submit_metadata(ScoopIoLoop *loop, int64_t id, ScoopIoFile *file, const char *path,
                           bool follow) {
    MetadataWork *request = calloc(1, sizeof(*request));
    if (!request)
        return -ENOMEM;
    request->file = file;
    request->follow = follow;
    if (path) {
        request->path = strdup(path);
        if (!request->path) {
            free(request);
            return -ENOMEM;
        }
    }
    int status = scoop_io_work_submit(&request->work, loop, id, execute_metadata, finish_metadata);
    if (status < 0) {
        free(request->path);
        free(request);
    }
    return status;
}

int scoop_io_file_metadata(ScoopIoFile *file, int64_t id) {
    return submit_metadata(file->loop, id, file, NULL, true);
}

int scoop_io_path_metadata(ScoopIoLoop *loop, int64_t id, const char *path, bool follow) {
    return submit_metadata(loop, id, NULL, path, follow);
}

ScoopPlatformFileMetadata scoop_io_metadata_take(ScoopPlatformFileMetadata *value) {
    ScoopPlatformFileMetadata result = *value;
    free(value);
    return result;
}
