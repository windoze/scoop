#include "../../../native/common/directory.h"
#include "../fence_work.h"
#include "loop_test.h"

int scoop_io_path_mkdir(ScoopIoLoop *loop, int64_t id, const wchar_t *path, int mode);
int scoop_io_path_delete(ScoopIoLoop *loop, int64_t id, const wchar_t *path);
int scoop_io_path_move(ScoopIoLoop *loop, int64_t id, const wchar_t *from, const wchar_t *to,
                       bool replace);
int scoop_io_resolve(ScoopIoLoop *loop, int64_t id, const wchar_t *host, int port, int family);

static wchar_t *child(const wchar_t *base, const wchar_t *name) {
    size_t length = wcslen(base), count = wcslen(name);
    wchar_t *path = calloc(length + count + 2, sizeof(wchar_t));
    if (path) {
        memcpy(path, base, length * sizeof(wchar_t));
        path[length] = L'\\';
        memcpy(path + length + 1, name, (count + 1) * sizeof(wchar_t));
    }
    return path;
}

static int completed(ScoopIoLoop *loop, int64_t id, int error) {
    ScoopIoEvent event;
    CHECK(!test_result(loop, &event) && event.id == id);
    CHECK(event.kind == (error ? SCOOP_IO_WIN32_ERROR : SCOOP_IO_DONE) && event.code == error);
    return 0;
}

static bool is_directory(const wchar_t *path) {
    DWORD attributes = GetFileAttributesW(path);
    return attributes != INVALID_FILE_ATTRIBUTES && (attributes & FILE_ATTRIBUTE_DIRECTORY);
}

int scoop_test_iocp_paths(const wchar_t *base) {
    ScoopIoCreated created = scoop_io_loop_create();
    CHECK(!created.status.code);
    ScoopIoLoop *loop = created.pointer;
    wchar_t *first = child(base, L"first-\xd800-\xd83d\xde00");
    wchar_t *second = child(base, L"second-\xdc00");
    CHECK(first && second);
    CHECK(!scoop_io_path_mkdir(loop, 1, first, 0700) && !completed(loop, 1, 0));
    CHECK(!scoop_io_path_mkdir(loop, 2, first, 0700) && !completed(loop, 2, ERROR_ALREADY_EXISTS));
    CHECK(!scoop_io_path_move(loop, 3, first, second, false) && !completed(loop, 3, 0));
    CHECK(GetFileAttributesW(first) == INVALID_FILE_ATTRIBUTES);
    CHECK(is_directory(second));
    CHECK(!scoop_io_path_delete(loop, 4, second) && !completed(loop, 4, 0));
    CHECK(!scoop_io_path_delete(loop, 5, second) && !completed(loop, 5, ERROR_FILE_NOT_FOUND));
    free(first);
    free(second);
    CHECK(!test_drain(loop));
    return 0;
}

int scoop_test_iocp_directory(const wchar_t *base) {
    const wchar_t *first_name = L"entry-\xd800-\xd83d\xde00", *second_name = L"entry-\xdc00";
    wchar_t *first = child(base, first_name), *second = child(base, second_name);
    CHECK(first && second);
    HANDLE file =
        CreateFileW(first, GENERIC_WRITE, 0, NULL, CREATE_NEW, FILE_ATTRIBUTE_NORMAL, NULL);
    CHECK(file != INVALID_HANDLE_VALUE && CloseHandle(file));
    file = CreateFileW(second, GENERIC_WRITE, 0, NULL, CREATE_NEW, FILE_ATTRIBUTE_NORMAL, NULL);
    CHECK(file != INVALID_HANDLE_VALUE && CloseHandle(file));
    ScoopIoCreated created = scoop_io_loop_create();
    CHECK(!created.status.code);
    ScoopIoLoop *loop = created.pointer;
    CHECK(!scoop_io_directory_open(loop, 1, base));
    ScoopIoEvent event;
    CHECK(!test_result(loop, &event) && event.kind == SCOOP_IO_RESOURCE);
    ScoopIoDirectory *directory = event.resource;
    wchar_t *names[2] = {0};
    for (int i = 0; i < 2; ++i) {
        CHECK(!scoop_io_directory_next(directory, 2 + i));
        CHECK(!test_result(loop, &event) && event.kind == SCOOP_IO_DIRECTORY_NAME);
        names[i] = event.resource;
    }
    CHECK(wcscmp(names[0], names[1]) != 0);
    CHECK((!wcscmp(names[0], first_name) && !wcscmp(names[1], second_name)) ||
          (!wcscmp(names[1], first_name) && !wcscmp(names[0], second_name)));
    CHECK(!scoop_io_directory_next(directory, 4));
    CHECK(!test_result(loop, &event) && event.kind == SCOOP_IO_EOF);
    CHECK(!scoop_io_directory_close(directory, 5) && !completed(loop, 5, 0));
    /* Returned names remain valid after the OS buffer and directory owner are gone. */
    CHECK(wcscmp(names[0], names[1]) != 0);
    scoop_io_directory_name_free(names[0]);
    scoop_io_directory_name_free(names[1]);
    CHECK(!test_drain(loop));
    CHECK(DeleteFileW(first) && DeleteFileW(second));
    free(first);
    free(second);
    return 0;
}

int scoop_test_iocp_queued_paths(const wchar_t *base) {
    ScoopIoCreated created = scoop_io_loop_create();
    CHECK(!created.status.code);
    ScoopIoLoop *loop = created.pointer;
    CHECK(!scoop_io_directory_open(loop, 1, base));
    ScoopIoEvent event;
    CHECK(!test_result(loop, &event) && event.kind == SCOOP_IO_RESOURCE);
    ScoopIoDirectory *directory = event.resource;
    created = scoop_test_fence_create(loop, 100, 0);
    CHECK(!created.status.code);
    TestWorkFence *fence = created.pointer;
    scoop_test_fence_wait(fence);
    wchar_t *path = child(base, L"queued-\xd800");
    CHECK(path);
    CHECK(!scoop_io_path_mkdir(loop, 2, path, 0700));
    CHECK(!scoop_io_resolve(loop, 3, L"127.0.0.1", 42, 4));
    CHECK(!scoop_io_directory_open(loop, 4, base));
    CHECK(!scoop_io_directory_next(directory, 5));
    for (int id = 2; id <= 5; ++id) {
        scoop_io_work_cancel(loop, id);
        CHECK(!test_result(loop, &event) && event.id == id && event.kind == SCOOP_IO_POSIX_ERROR &&
              event.code == ECANCELED);
    }
    CHECK(GetFileAttributesW(path) == INVALID_FILE_ATTRIBUTES);
    CHECK(!scoop_io_path_mkdir(loop, 6, path, 0700));
    wchar_t saved = path[0];
    path[0] = L'!';
    scoop_test_fence_release(fence);
    unsigned seen = 0;
    for (unsigned i = 0; i < 2; ++i) {
        CHECK(!test_result(loop, &event) && event.kind == SCOOP_IO_DONE);
        unsigned bit = event.id == 6 ? 1u : event.id == 100 ? 2u : 0u;
        CHECK(bit && !(seen & bit));
        seen |= bit;
    }
    path[0] = saved;
    CHECK(is_directory(path));
    CHECK(!scoop_io_directory_close(directory, 7) && !completed(loop, 7, 0));
    CHECK(!scoop_io_path_delete(loop, 8, path) && !completed(loop, 8, 0));
    free(path);
    CHECK(!test_drain(loop));
    return 0;
}
