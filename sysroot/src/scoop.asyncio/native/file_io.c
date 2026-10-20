#include "file.h"
#include <errno.h>

typedef enum {
    FILE_READ,
    FILE_WRITE,
    FILE_SYNC_DATA,
    FILE_SYNC_ALL,
    FILE_POSITION,
    FILE_SEEK,
    FILE_TRUNCATE
} FileAction;
typedef struct {
    ScoopIoWork work;
    ScoopIoFile *file;
    FileAction action;
    void *bytes;
    int64_t count;
} ScoopIoFileWork;

static void file_execute(uv_work_t *native) {
    ScoopIoFileWork *request = native->data;
    int fd = request->file->fd;
    int64_t result;
    switch (request->action) {
    case FILE_READ:
        result = scoop_platform_fd_read(fd, request->bytes, request->count);
        break;
    case FILE_WRITE:
        result = scoop_platform_fd_write(fd, request->bytes, request->count);
        break;
    case FILE_SYNC_DATA:
    case FILE_SYNC_ALL:
        result = scoop_platform_file_sync(fd, request->action == FILE_SYNC_DATA);
        break;
    case FILE_POSITION:
    case FILE_SEEK:
        result = scoop_platform_file_seek(fd, request->count, request->action == FILE_POSITION);
        break;
    case FILE_TRUNCATE:
        result = scoop_platform_file_truncate(fd, request->count);
        break;
    default:
        abort();
    }
    int error = errno;
    ScoopIoEvent *event = &request->work.completion.event;
    if (result < 0) {
        event->kind = SCOOP_IO_POSIX_ERROR;
        event->code = error;
    } else if (request->action == FILE_READ || request->action == FILE_WRITE) {
        event->kind = SCOOP_IO_BYTES;
        event->count = result;
    } else if (request->action == FILE_POSITION || request->action == FILE_SEEK) {
        event->kind = SCOOP_IO_POSITION;
        event->count = result;
    }
}

static void file_finished(uv_work_t *native, int status) {
    ScoopIoFileWork *request = native->data;
    scoop_io_work_finished(&request->work, status);
}

static int submit_file(ScoopIoFile *file, int64_t id, FileAction action, void *bytes, int64_t count) {
    ScoopIoFileWork *request = calloc(1, sizeof(*request));
    if (!request)
        return UV_ENOMEM;
    request->file = file;
    request->action = action;
    request->bytes = bytes;
    request->count = count;
    int status = scoop_io_work_submit(&request->work, file->loop, id, file_execute, file_finished);
    if (status < 0)
        free(request);
    return status;
}

int scoop_io_file_transfer(ScoopIoFile *file, int64_t id, void *bytes, int64_t count, bool writing) {
    return submit_file(file, id, writing ? FILE_WRITE : FILE_READ, bytes, count);
}

int scoop_io_file_sync(ScoopIoFile *file, int64_t id, bool data_only) {
    return submit_file(file, id, data_only ? FILE_SYNC_DATA : FILE_SYNC_ALL, NULL, 0);
}

int scoop_io_file_seek(ScoopIoFile *file, int64_t id, int64_t offset, bool current) {
    return submit_file(file, id, current ? FILE_POSITION : FILE_SEEK, NULL, offset);
}

int scoop_io_file_truncate(ScoopIoFile *file, int64_t id, int64_t size) {
    return submit_file(file, id, FILE_TRUNCATE, NULL, size);
}

static void close_execute(uv_work_t *native) {
    ScoopIoFile *file = native->data;
    int result = scoop_platform_fd_close(file->fd);
    int error = errno;
    file->fd = -1;
    if (result < 0) {
        file->closing.completion.event.kind = SCOOP_IO_POSIX_ERROR;
        file->closing.completion.event.code = error;
    }
}

static void close_finished(uv_work_t *native, int status) {
    ScoopIoFile *file = native->data;
    scoop_io_work_finished(&file->closing, status);
    /* The completion queue releases the allocation containing close work. */
}

int scoop_io_file_close(ScoopIoFile *file, int64_t id) {
    int status = scoop_io_work_submit(&file->closing, file->loop, id, close_execute, close_finished);
    /* In pinned libuv, valid non-null callbacks make queue_work infallible. */
    if (status < 0)
        abort();
    return 0;
}
