#include "loop_test.h"
#include <stdio.h>
#include <string.h>

typedef struct {
    HANDLE input, output;
} Pipe;

static Pipe pipe_open(ScoopIoLoop *loop) {
    static LONG sequence;
    wchar_t name[128];
    swprintf(name, 128, L"\\\\.\\pipe\\scoop-iocp-%lu-%ld", GetCurrentProcessId(),
             InterlockedIncrement(&sequence));
    HANDLE input = CreateNamedPipeW(name, PIPE_ACCESS_INBOUND | FILE_FLAG_OVERLAPPED,
                                    PIPE_TYPE_BYTE | PIPE_WAIT, 1, 4096, 4096, 0, NULL);
    if (input == INVALID_HANDLE_VALUE)
        return (Pipe){NULL, NULL};
    HANDLE output = CreateFileW(name, GENERIC_WRITE, 0, NULL, OPEN_EXISTING, 0, NULL);
    if (output == INVALID_HANDLE_VALUE ||
        CreateIoCompletionPort(input, loop->port, 0, 0) != loop->port) {
        if (output != INVALID_HANDLE_VALUE)
            CloseHandle(output);
        CloseHandle(input);
        return (Pipe){NULL, NULL};
    }
    return (Pipe){input, output};
}

static void read_finished(ScoopIoRequest *request, DWORD count, DWORD error) {
    scoop_io_request_retire(request);
    request->completion.event.kind = error ? SCOOP_IO_WIN32_ERROR : SCOOP_IO_BYTES;
    request->completion.event.code = (int32_t)error;
    request->completion.event.count = count;
    scoop_io_complete(request->loop, &request->completion, 0);
}

static int read_start(ScoopIoLoop *loop, HANDLE input, int64_t id, void *bytes, bool *immediate) {
    ScoopIoRequest *request = calloc(1, sizeof(*request));
    if (!request)
        return -1;
    scoop_io_request_init(request, loop, id, input, read_finished);
    *immediate = ReadFile(input, bytes, 16, NULL, &request->overlapped) != 0;
    DWORD error = *immediate ? 0 : GetLastError();
    if (error && error != ERROR_IO_PENDING)
        read_finished(request, 0, error);
    return 0;
}

int scoop_test_iocp_requests(bool prefill) {
    ScoopIoCreated created = scoop_io_loop_create();
    CHECK(!created.status.code);
    ScoopIoLoop *loop = created.pointer;
    Pipe pipe = pipe_open(loop);
    CHECK(pipe.input && pipe.output);
    unsigned char buffer[16] = {0};
    bool immediate;
    CHECK(!read_start(loop, pipe.input, 1, buffer, &immediate) && !immediate);
    CHECK(loop->requests && !scoop_io_loop_batch_size(loop));
    scoop_io_request_cancel(loop, 1);
    CHECK(loop->requests && !scoop_io_loop_batch_size(loop));
    ScoopIoEvent event;
    CHECK(!test_result(loop, &event));
    CHECK(event.id == 1 && event.kind == SCOOP_IO_WIN32_ERROR &&
          event.code == ERROR_OPERATION_ABORTED);
    CHECK(!loop->requests);

    const char data[] = "iocp bytes";
    DWORD written;
    if (prefill)
        CHECK(WriteFile(pipe.output, data, sizeof(data), &written, NULL) &&
              written == sizeof(data));
    CHECK(!read_start(loop, pipe.input, 2, buffer, &immediate));
    CHECK(immediate == prefill);
    CHECK(loop->requests && !scoop_io_loop_batch_size(loop));
    if (!prefill)
        CHECK(WriteFile(pipe.output, data, sizeof(data), &written, NULL) &&
              written == sizeof(data));
    /* Retire an old event hint before waiting for this request's actual native packet. */
    AcquireSRWLockExclusive(&loop->mutex);
    bool queued = loop->packets_head != NULL;
    if (!queued)
        ResetEvent(loop->notification);
    ReleaseSRWLockExclusive(&loop->mutex);
    CHECK(queued || !test_wait(loop));
    AcquireSRWLockExclusive(&loop->mutex);
    queued = loop->packets_head != NULL;
    ReleaseSRWLockExclusive(&loop->mutex);
    CHECK(queued && loop->requests && !scoop_io_loop_batch_size(loop));
    scoop_io_request_cancel(loop, 2);
    CHECK(loop->requests);
    CHECK(!test_result(loop, &event));
    CHECK(event.id == 2 && event.kind == SCOOP_IO_BYTES && event.count == sizeof(data));
    CHECK(!memcmp(buffer, data, sizeof(data)) && !loop->requests);
    CHECK(!scoop_io_loop_poll(loop, false).code && !scoop_io_loop_batch_size(loop));
    CHECK(CloseHandle(pipe.output) && CloseHandle(pipe.input));
    CHECK(!test_drain(loop));
    return 0;
}
