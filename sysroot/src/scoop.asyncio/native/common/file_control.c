#include "file.h"
#include <errno.h>

typedef struct {
    ScoopIoWork work;
    ScoopIoFile *file;
    int64_t offset;
    bool truncate, current;
} FileControl;

static void execute(ScoopIoWork *native) {
    FileControl *request = (void *)native;
    int64_t result =
        request->truncate
            ? scoop_platform_file_truncate(request->file->fd, request->offset)
            : scoop_platform_file_seek(request->file->fd, request->offset, request->current);
    int error = errno;
    if (result < 0) {
        native->completion.event.kind = SCOOP_IO_POSIX_ERROR;
        native->completion.event.code = error;
    } else if (!request->truncate) {
        native->completion.event.kind = SCOOP_IO_POSITION;
        native->completion.event.count = result;
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
    int status = scoop_platform_fd_close(file->fd);
    int error = errno;
    file->fd = -1;
    if (status < 0) {
        native->completion.event.kind = SCOOP_IO_POSIX_ERROR;
        native->completion.event.code = error;
    }
}
int scoop_io_file_close(ScoopIoFile *file, int64_t id) {
    // An opened file already has a live worker set; this preallocated work cannot fail to queue.
    return scoop_io_work_submit(&file->closing, file->loop, id, close_execute, finish);
}
