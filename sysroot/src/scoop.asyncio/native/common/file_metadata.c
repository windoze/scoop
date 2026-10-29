#include "file.h"
#include <errno.h>
#include <string.h>

typedef struct {
    ScoopIoWork work;
    ScoopIoFile *file;
    ScoopIoChar *path;
    bool follow;
} MetadataWork;

static void execute_metadata(ScoopIoWork *native) {
    MetadataWork *request = (void *)native;
    ScoopPlatformFileMetadataResult data =
        request->file ? scoop_platform_handle_metadata(request->file->handle)
                      : scoop_platform_path_metadata_result(request->path, request->follow);
    ScoopIoEvent *event = &request->work.completion.event;
    if (data.error) {
        scoop_io_platform_error(event, data.error);
        return;
    }
    ScoopPlatformFileMetadata *result = malloc(sizeof(*result));
    if (!result) {
        event->kind = SCOOP_IO_POSIX_ERROR;
        event->code = ENOMEM;
        return;
    }
    *result = data.data;
    event->kind = SCOOP_IO_METADATA;
    event->resource = result;
}

static void finish_metadata(ScoopIoWork *native, int status) {
    MetadataWork *request = (void *)native;
    free(request->path);
    scoop_io_work_finished(&request->work, status);
}

static int submit_metadata(ScoopIoLoop *loop, int64_t id, ScoopIoFile *file,
                           const ScoopIoChar *path, bool follow) {
    MetadataWork *request = calloc(1, sizeof(*request));
    if (!request)
        return -ENOMEM;
    request->file = file;
    request->follow = follow;
    if (path) {
        request->path = scoop_io_string_copy(path);
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

int scoop_io_path_metadata(ScoopIoLoop *loop, int64_t id, const ScoopIoChar *path, bool follow) {
    return submit_metadata(loop, id, NULL, path, follow);
}

ScoopPlatformFileMetadata scoop_io_metadata_take(ScoopPlatformFileMetadata *value) {
    ScoopPlatformFileMetadata result = *value;
    free(value);
    return result;
}
