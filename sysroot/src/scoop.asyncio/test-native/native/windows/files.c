#include "../../../native/common/file.h"
#include "../fence_work.h"
#include "loop_test.h"
#include <string.h>
#include <stdio.h>

static int completed(ScoopIoLoop *loop, int kind, int64_t count) {
    ScoopIoEvent event;
    CHECK(!test_result(loop, &event) && event.kind == kind && event.count == count);
    return 0;
}

static int opened(ScoopIoLoop *loop, const wchar_t *path, int flags, ScoopIoFile **file) {
    CHECK(!scoop_io_file_open(loop, 1, path, flags, 0600));
    ScoopIoEvent event;
    CHECK(!test_result(loop, &event) && event.kind == SCOOP_IO_RESOURCE);
    *file = event.resource;
    DWORD attributes;
    CHECK(GetHandleInformation(scoop_file_handle((*file)->handle), &attributes) &&
          !(attributes & HANDLE_FLAG_INHERIT));
    return 0;
}

static int position(ScoopIoFile *file, int64_t expected) {
    CHECK(!scoop_io_file_seek(file, 2, 0, true));
    return completed(file->loop, SCOOP_IO_POSITION, expected);
}

static int closed(ScoopIoFile *file) {
    ScoopIoLoop *loop = file->loop;
    CHECK(!scoop_io_file_close(file, 3));
    return completed(loop, SCOOP_IO_DONE, 0);
}

int scoop_test_iocp_file_controls(const wchar_t *path) {
    ScoopIoCreated created = scoop_io_loop_create();
    CHECK(!created.status.code);
    ScoopIoLoop *loop = created.pointer;
    ScoopIoFile *file = NULL;
    CHECK(!opened(loop, path, 1 | 2 | 16, &file) && file->overlapped);
    unsigned char bytes[6] = {0, 13, 10, 0x1a, 0xff, 0x80};
    CHECK(!scoop_io_file_transfer(file, 4, bytes, sizeof(bytes), true));
    CHECK(loop->requests && file->position == 0);
    CHECK(!completed(loop, SCOOP_IO_BYTES, sizeof(bytes)) && !position(file, sizeof(bytes)));
    CHECK(!scoop_io_file_seek(file, 5, 0, false) && !completed(loop, SCOOP_IO_POSITION, 0));
    unsigned char read[9] = {0};
    CHECK(!scoop_io_file_transfer(file, 6, read, sizeof(read), false));
    CHECK(!completed(loop, SCOOP_IO_BYTES, sizeof(bytes)) && !memcmp(bytes, read, sizeof(bytes)));
    CHECK(!position(file, sizeof(bytes)));
    CHECK(!scoop_io_file_transfer(file, 7, read, sizeof(read), false));
    CHECK(!completed(loop, SCOOP_IO_BYTES, 0) && !position(file, sizeof(bytes)));
    const int64_t large = INT64_C(4294967297);
    CHECK(!scoop_io_file_seek(file, 8, large, false) && !completed(loop, SCOOP_IO_POSITION, large));
    CHECK(!scoop_io_file_truncate(file, 9, 2) && !completed(loop, SCOOP_IO_DONE, 0));
    CHECK(!position(file, large));
    CHECK(!scoop_io_file_metadata(file, 10));
    ScoopIoEvent event;
    CHECK(!test_result(loop, &event) && event.kind == SCOOP_IO_METADATA);
    ScoopPlatformFileMetadata metadata = scoop_io_metadata_take(event.resource);
    CHECK(metadata.kind == 0 && metadata.size == 2 && metadata.permissions == 0666);
    CHECK(!scoop_io_file_sync(file, 11, true) && !completed(loop, SCOOP_IO_DONE, 0));
    CHECK(!scoop_io_file_sync(file, 12, false) && !completed(loop, SCOOP_IO_DONE, 0));
    CHECK(!closed(file));
    CHECK(!opened(loop, path, 1, &file));
    CHECK(!scoop_io_file_transfer(file, 13, bytes, sizeof(bytes), true));
    CHECK(!test_result(loop, &event) && event.kind == SCOOP_IO_WIN32_ERROR &&
          event.code == ERROR_ACCESS_DENIED);
    CHECK(!scoop_io_file_truncate(file, 14, 0));
    CHECK(!test_result(loop, &event) && event.kind == SCOOP_IO_WIN32_ERROR &&
          event.code == ERROR_ACCESS_DENIED);
    CHECK(!position(file, 0));
    CHECK(!scoop_io_file_transfer(file, 15, read, sizeof(read), false));
    CHECK(!completed(loop, SCOOP_IO_BYTES, 2) && !memcmp(bytes, read, 2));
    CHECK(!closed(file) && !test_drain(loop));
    return 0;
}

int scoop_test_iocp_file_append(const wchar_t *path) {
    ScoopIoCreated created = scoop_io_loop_create();
    CHECK(!created.status.code);
    ScoopIoLoop *loop = created.pointer;
    ScoopIoFile *first = NULL, *second = NULL;
    CHECK(!opened(loop, path, 1 | 2 | 4 | 16, &first) && !first->overlapped);
    CHECK(!opened(loop, path, 1 | 2 | 4, &second) && !second->overlapped);
    unsigned char bytes[] = {1, 2, 3};
    CHECK(!scoop_io_file_transfer(first, 4, bytes, 3, true) && !completed(loop, SCOOP_IO_BYTES, 3));
    CHECK(!scoop_io_file_transfer(second, 5, bytes, 1, true) &&
          !completed(loop, SCOOP_IO_BYTES, 1));
    CHECK(!position(first, 3) && !position(second, 4));
    CHECK(!scoop_io_file_seek(first, 6, 0, false) && !completed(loop, SCOOP_IO_POSITION, 0));
    CHECK(!scoop_io_file_transfer(first, 7, bytes, 2, true) && !completed(loop, SCOOP_IO_BYTES, 2));
    CHECK(!position(first, 6) && !position(second, 4));
    CHECK(!scoop_io_file_seek(first, 8, 0, false) && !completed(loop, SCOOP_IO_POSITION, 0));
    unsigned char read[6] = {0}, expected[] = {1, 2, 3, 1, 1, 2};
    CHECK(!scoop_io_file_transfer(first, 9, read, sizeof(read), false));
    CHECK(!completed(loop, SCOOP_IO_BYTES, 6) && !memcmp(read, expected, sizeof(read)));
    created = scoop_test_fence_create(loop, 100, 0);
    CHECK(!created.status.code);
    TestWorkFence *fence = created.pointer;
    scoop_test_fence_wait(fence);
    CHECK(!scoop_io_file_transfer(first, 10, bytes, 3, true));
    scoop_io_work_cancel(loop, 10);
    ScoopIoEvent event;
    CHECK(!test_result(loop, &event) && event.id == 10 && event.kind == SCOOP_IO_POSIX_ERROR &&
          event.code == ECANCELED);
    scoop_test_fence_release(fence);
    CHECK(!completed(loop, SCOOP_IO_DONE, 0));
    CHECK(!position(first, 6));
    CHECK(!closed(first) && !closed(second) && !test_drain(loop));
    return 0;
}

int scoop_test_iocp_file_cancel(void) {
    wchar_t name[100];
    CHECK(swprintf(name, 100, L"\\\\.\\pipe\\scoop-iocp-file-%lu-%llu", GetCurrentProcessId(),
                   (unsigned long long)GetTickCount64()) > 0);
    HANDLE server = CreateNamedPipeW(name, PIPE_ACCESS_DUPLEX | FILE_FLAG_OVERLAPPED,
                                     PIPE_TYPE_BYTE | PIPE_WAIT, 1, 4096, 4096, 0, NULL);
    CHECK(server != INVALID_HANDLE_VALUE);
    ScoopIoCreated created = scoop_io_loop_create();
    CHECK(!created.status.code);
    ScoopIoLoop *loop = created.pointer;
    ScoopIoFile *file = NULL;
    CHECK(!opened(loop, name, 1 | 2, &file));
    unsigned char byte = 0, sent = 0xfe;
    CHECK(!scoop_io_file_transfer(file, 4, &byte, 1, false));
    scoop_io_work_cancel(loop, 4);
    CHECK(loop->requests);
    ScoopIoEvent event;
    CHECK(!test_result(loop, &event) && event.id == 4 && event.kind == SCOOP_IO_WIN32_ERROR &&
          event.code == ERROR_OPERATION_ABORTED);
    CHECK(!scoop_io_file_transfer(file, 5, &byte, 1, false));
    OVERLAPPED output = {.hEvent = CreateEventW(NULL, TRUE, FALSE, NULL)};
    CHECK(output.hEvent);
    DWORD count = 0;
    CHECK(WriteFile(server, &sent, 1, NULL, &output) || GetLastError() == ERROR_IO_PENDING);
    CHECK(GetOverlappedResult(server, &output, &count, TRUE) && count == 1);
    CHECK(loop->requests && loop->requests->completion.event.id == 5);
    CHECK(GetOverlappedResult(scoop_file_handle(file->handle), &loop->requests->overlapped, &count,
                              TRUE) &&
          count == 1);
    scoop_io_work_cancel(loop, 5);
    CHECK(!test_result(loop, &event) && event.id == 5 && event.kind == SCOOP_IO_BYTES &&
          event.count == 1 && byte == sent);
    CHECK(!scoop_io_file_seek(file, 6, 0, true));
    CHECK(!test_result(loop, &event) && event.kind == SCOOP_IO_WIN32_ERROR && event.code != 0);
    CHECK(CloseHandle(output.hEvent) && CloseHandle(server));
    CHECK(!scoop_io_file_transfer(file, 7, &byte, 1, false) && !completed(loop, SCOOP_IO_BYTES, 0));
    CHECK(!closed(file) && !test_drain(loop));
    return 0;
}

int scoop_test_iocp_file_queued(const wchar_t *path) {
    ScoopIoCreated created = scoop_io_loop_create();
    CHECK(!created.status.code);
    ScoopIoLoop *loop = created.pointer;
    created = scoop_test_fence_create(loop, 100, 0);
    CHECK(!created.status.code);
    TestWorkFence *fence = created.pointer;
    scoop_test_fence_wait(fence);
    CHECK(!scoop_io_file_open(loop, 1, path, 1 | 2 | 16, 0600));
    scoop_io_work_cancel(loop, 1);
    ScoopIoEvent event;
    CHECK(!test_result(loop, &event) && event.id == 1 && event.kind == SCOOP_IO_POSIX_ERROR &&
          event.code == ECANCELED);
    CHECK(GetFileAttributesW(path) == INVALID_FILE_ATTRIBUTES);
    scoop_test_fence_release(fence);
    CHECK(!completed(loop, SCOOP_IO_DONE, 0));
    CHECK(!scoop_io_file_open(loop, 2, path, 1 | 2 | 16, 0600));
    created = scoop_test_fence_create(loop, 101, 0);
    CHECK(!created.status.code);
    fence = created.pointer;
    scoop_test_fence_wait(fence);
    scoop_io_work_cancel(loop, 2);
    CHECK(!test_result(loop, &event) && event.id == 2 && event.kind == SCOOP_IO_RESOURCE);
    ScoopIoFile *file = event.resource;
    DWORD attributes;
    CHECK(GetHandleInformation(scoop_file_handle(file->handle), &attributes));
    scoop_test_fence_release(fence);
    CHECK(!completed(loop, SCOOP_IO_DONE, 0));
    CHECK(!closed(file) && !test_drain(loop));
    return 0;
}
