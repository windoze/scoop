#include "../common/file.h"
#include "backend.h"
#include <errno.h>
#include <limits.h>

typedef struct {
    ScoopIoRequest request;
    ScoopIoFile *file;
    void *bytes;
    unsigned count;
    bool writing, syncing, data_only;
} FileIo;

static void prepare(ScoopIoRequest *native, unsigned index, struct io_uring_sqe *sqe) {
    (void)index;
    FileIo *request = (void *)native;
    if (request->syncing)
        io_uring_prep_fsync(sqe, request->file->fd, request->data_only ? IORING_FSYNC_DATASYNC : 0);
    else if (request->writing)
        io_uring_prep_write(sqe, request->file->fd, request->bytes, request->count, (__u64)-1);
    else
        io_uring_prep_read(sqe, request->file->fd, request->bytes, request->count, (__u64)-1);
    sqe->flags |= IOSQE_ASYNC;
}
static void result(ScoopIoRequest *native, unsigned index, int status) {
    (void)index;
    FileIo *request = (void *)native;
    if (status < 0)
        scoop_io_request_error(native, -status);
    else {
        if (!request->syncing) {
            native->completion.event.kind = SCOOP_IO_BYTES;
            native->completion.event.count = status;
        }
        scoop_io_request_end(native);
    }
}
static void finish(ScoopIoRequest *request) {
    scoop_io_complete(request->loop, &request->completion, 0);
}
static int submit(ScoopIoFile *file, int64_t id, void *bytes, int64_t count, bool writing,
                  bool syncing, bool data_only) {
    FileIo *request = calloc(1, sizeof(*request));
    if (!request)
        return -ENOMEM;
    request->file = file;
    request->bytes = bytes;
    request->count = count > INT_MAX ? INT_MAX : (unsigned)count;
    request->writing = writing;
    request->syncing = syncing;
    request->data_only = data_only;
    request->request.prepare = prepare;
    request->request.result = result;
    request->request.finish = finish;
    scoop_io_request_init(&request->request, file->loop, id);
    scoop_io_request_queue(&request->request, 0);
    return 0;
}
int scoop_io_file_transfer(ScoopIoFile *file, int64_t id, void *bytes, int64_t count,
                           bool writing) {
    return submit(file, id, bytes, count, writing, false, false);
}
int scoop_io_file_sync(ScoopIoFile *file, int64_t id, bool data_only) {
    return submit(file, id, NULL, 0, false, true, data_only);
}
