#include "file.h"
#include <errno.h>

typedef enum { FILE_READ, FILE_WRITE, FILE_SYNC_DATA, FILE_SYNC_ALL } FileAction;
typedef struct {
    ScoopIoWork work;
    ScoopIoFile *file;
    FileAction action;
    void *bytes;
    int64_t count;
} FileIo;

static void execute(ScoopIoWork *native) {
    FileIo *request = (void *)native;
    ScoopPlatformFileHandle handle = request->file->handle;
    ScoopPlatformFileResult result = {0};
    switch (request->action) {
    case FILE_READ:
        result = scoop_platform_handle_read(handle, request->bytes, request->count);
        break;
    case FILE_WRITE:
        result = scoop_platform_handle_write(handle, request->bytes, request->count);
        break;
    case FILE_SYNC_DATA:
    case FILE_SYNC_ALL:
        result = scoop_platform_handle_sync(handle, request->action == FILE_SYNC_DATA);
        break;
    }
    if (result.error)
        scoop_io_platform_error(&native->completion.event, result.error);
    else if (request->action == FILE_READ || request->action == FILE_WRITE) {
        native->completion.event.kind = SCOOP_IO_BYTES;
        native->completion.event.count = result.value;
    }
}
static void finish(ScoopIoWork *work, int status) { scoop_io_work_finished(work, status); }
static int submit(ScoopIoFile *file, int64_t id, FileAction action, void *bytes, int64_t count) {
    FileIo *request = calloc(1, sizeof(*request));
    if (!request)
        return -ENOMEM;
    request->file = file;
    request->action = action;
    request->bytes = bytes;
    request->count = count;
    int status = scoop_io_work_submit(&request->work, file->loop, id, execute, finish);
    if (status < 0)
        free(request);
    return status;
}
int scoop_io_file_transfer_work(ScoopIoFile *file, int64_t id, void *bytes, int64_t count,
                                bool writing) {
    return submit(file, id, writing ? FILE_WRITE : FILE_READ, bytes, count);
}
#ifndef _WIN32
int scoop_io_file_transfer(ScoopIoFile *file, int64_t id, void *bytes, int64_t count,
                           bool writing) {
    return scoop_io_file_transfer_work(file, id, bytes, count, writing);
}
#endif
int scoop_io_file_sync(ScoopIoFile *file, int64_t id, bool data_only) {
    return submit(file, id, data_only ? FILE_SYNC_DATA : FILE_SYNC_ALL, NULL, 0);
}
