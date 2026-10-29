#include "file.h"
#include <errno.h>

typedef struct {
    ScoopIoWork work;
    ScoopIoFile *file;
    int64_t offset;
    bool truncate, current;
} FileControl;

static ScoopPlatformFileResult seek_file(FileControl *request) {
    ScoopIoFile *file = request->file;
    int64_t offset = request->offset;
    bool current = request->current;
#ifdef _WIN32
    if (file->overlapped && current) {
        if (offset > INT64_MAX - file->position)
            return (ScoopPlatformFileResult){-1, ERROR_ARITHMETIC_OVERFLOW};
        offset += file->position;
        current = false;
    }
#endif
    ScoopPlatformFileResult result = scoop_platform_handle_seek(file->handle, offset, current);
#ifdef _WIN32
    if (!result.error && file->overlapped)
        file->position = result.value;
#endif
    return result;
}

static void execute(ScoopIoWork *native) {
    FileControl *request = (void *)native;
    ScoopPlatformFileResult result =
        request->truncate ? scoop_platform_handle_truncate(request->file->handle, request->offset)
                          : seek_file(request);
    if (result.error)
        scoop_io_platform_error(&native->completion.event, result.error);
    else if (!request->truncate) {
        native->completion.event.kind = SCOOP_IO_POSITION;
        native->completion.event.count = result.value;
    }
}
static void finish(ScoopIoWork *work, int status) { scoop_io_work_finished(work, status); }
static int submit(ScoopIoFile *file, int64_t id, int64_t offset, bool current, bool truncate) {
    FileControl *request = calloc(1, sizeof(*request));
    if (!request)
        return -ENOMEM;
    request->file = file;
    request->offset = offset;
    request->current = current;
    request->truncate = truncate;
    int status = scoop_io_work_submit(&request->work, file->loop, id, execute, finish);
    if (status < 0)
        free(request);
    return status;
}
int scoop_io_file_seek(ScoopIoFile *file, int64_t id, int64_t offset, bool current) {
    return submit(file, id, offset, current, false);
}
int scoop_io_file_truncate(ScoopIoFile *file, int64_t id, int64_t size) {
    return submit(file, id, size, false, true);
}
static void close_execute(ScoopIoWork *native) {
    ScoopIoFile *file = (void *)native;
    ScoopPlatformFileResult result = scoop_platform_handle_close(file->handle);
    file->handle.value = UINT64_MAX;
    scoop_io_platform_error(&native->completion.event, result.error);
}
int scoop_io_file_close(ScoopIoFile *file, int64_t id) {
    // An opened file already has a live worker set; this preallocated work cannot fail to queue.
    return scoop_io_work_submit(&file->closing, file->loop, id, close_execute, finish);
}
