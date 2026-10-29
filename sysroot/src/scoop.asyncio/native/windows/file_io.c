#include "backend.h"
#include "../common/file.h"
#include <errno.h>

typedef struct {
    ScoopIoRequest request;
    ScoopIoFile *file;
    bool writing;
} FileIo;

int scoop_io_file_associate(ScoopIoFile *file) {
    return CreateIoCompletionPort(scoop_file_handle(file->handle), file->loop->port, 0, 0)
               ? 0
               : (int)GetLastError();
}

static void finish(ScoopIoRequest *native, DWORD count, DWORD error) {
    FileIo *request = (void *)native;
    ScoopIoFile *file = request->file;
    if (!request->writing && (error == ERROR_HANDLE_EOF || error == ERROR_BROKEN_PIPE))
        error = 0;
    ScoopIoEvent *event = &native->completion.event;
    event->kind = SCOOP_IO_BYTES;
    event->count = count;
    if (count <= (uint64_t)(INT64_MAX - file->position))
        file->position += count;
    else if (!error)
        error = ERROR_ARITHMETIC_OVERFLOW;
    scoop_io_platform_error(event, (int)error);
    scoop_io_request_retire(native);
    scoop_io_complete(native->loop, &native->completion, 0);
}

int scoop_io_file_transfer(ScoopIoFile *file, int64_t id, void *bytes, int64_t count,
                           bool writing) {
    if (!file->overlapped)
        return scoop_io_file_transfer_work(file, id, bytes, count, writing);
    FileIo *request = calloc(1, sizeof(*request));
    if (!request)
        return -ENOMEM;
    request->file = file;
    request->writing = writing;
    HANDLE handle = scoop_file_handle(file->handle);
    scoop_io_request_init(&request->request, file->loop, id, handle, finish);
    OVERLAPPED *overlapped = &request->request.overlapped;
    overlapped->Offset = (DWORD)file->position;
    overlapped->OffsetHigh = (DWORD)((uint64_t)file->position >> 32);
    DWORD length = (uint64_t)count > UINT32_MAX ? UINT32_MAX : (DWORD)count;
    BOOL success = writing ? WriteFile(handle, bytes, length, NULL, overlapped)
                           : ReadFile(handle, bytes, length, NULL, overlapped);
    if (!success) {
        DWORD error = GetLastError();
        if (error != ERROR_IO_PENDING)
            finish(&request->request, 0, error);
    }
    return 0;
}
