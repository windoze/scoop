#include "../common/temporary.h"
#include "../../../scoop.platform/native/path_strings.h"
#include "../../../scoop.platform/native/random.h"
#include <errno.h>

typedef struct {
    ScoopIoWork work;
    ScoopIoTemporaryFile *result;
    wchar_t *parent, *name;
} TemporaryWork;

static ScoopIoStatus create_temporary(TemporaryWork *request) {
    ScoopPlatformString parent = scoop_platform_path_canonical_result(request->parent);
    if (parent.error)
        return (ScoopIoStatus){SCOOP_IO_WIN32_ERROR, parent.error};
    size_t base = (size_t)parent.length, name = wcslen(request->name);
    if (name < 6 || name > SIZE_MAX / sizeof(wchar_t) - 2 ||
        base > SIZE_MAX / sizeof(wchar_t) - name - 2) {
        free(parent.data);
        return (ScoopIoStatus){SCOOP_IO_WIN32_ERROR, ERROR_INVALID_PARAMETER};
    }
    ScoopIoTemporaryFile *result = request->result;
    result->path = calloc(base + name + 2, sizeof(wchar_t));
    if (!result->path) {
        free(parent.data);
        return scoop_io_errno(-ENOMEM);
    }
    memcpy(result->path, parent.data, base * sizeof(wchar_t));
    free(parent.data);
    if (!base || result->path[base - 1] != L'\\')
        result->path[base++] = L'\\';
    memcpy(result->path + base, request->name, (name + 1) * sizeof(wchar_t));
    for (;;) {
        uint64_t random = 0;
        struct ScoopPlatformEntropy entropy = scoop_platform_entropy(&random, sizeof(random));
        if (entropy.error)
            return (ScoopIoStatus){SCOOP_IO_NTSTATUS_ERROR, entropy.error};
        static const wchar_t letters[] =
            L"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
        for (size_t index = base + name - 6; index < base + name; ++index) {
            result->path[index] = letters[random % 62];
            random /= 62;
        }
        ScoopPlatformFileOpened opened =
            scoop_platform_handle_open_overlapped(result->path, 1 | 2 | 16, 0600);
        if (opened.error == ERROR_FILE_EXISTS || opened.error == ERROR_ALREADY_EXISTS)
            continue;
        if (opened.error)
            return (ScoopIoStatus){SCOOP_IO_WIN32_ERROR, opened.error};
        result->file->handle = opened.handle;
        result->file->overlapped = true;
        return (ScoopIoStatus){SCOOP_IO_WIN32_ERROR, scoop_io_file_associate(result->file)};
    }
}

static void execute(ScoopIoWork *native) {
    TemporaryWork *request = (void *)native;
    ScoopIoTemporaryFile *result = request->result;
    result->status = create_temporary(request);
    ScoopIoEvent *event = &native->completion.event;
    if (result->file->handle.value != UINT64_MAX) {
        /* Even association failure must hand the owned path to managed close/delete cleanup. */
        event->kind = SCOOP_IO_TEMPORARY;
        event->resource = result;
    } else {
        event->kind = result->status.domain;
        event->code = result->status.code;
    }
}

static void finish(ScoopIoWork *native, int status) {
    TemporaryWork *request = (void *)native;
    free(request->parent);
    free(request->name);
    if (request->result->file->handle.value == UINT64_MAX)
        scoop_io_temporary_free(request->result);
    scoop_io_work_finished(native, status);
}

int scoop_io_file_temporary(ScoopIoLoop *loop, int64_t id, const wchar_t *parent,
                            const wchar_t *name) {
    TemporaryWork *request = calloc(1, sizeof(*request));
    if (!request)
        return -ENOMEM;
    request->result = calloc(1, sizeof(*request->result));
    if (!request->result) {
        free(request);
        return -ENOMEM;
    }
    request->parent = scoop_io_string_copy(parent);
    request->name = scoop_io_string_copy(name);
    request->result->file = scoop_io_file_new(loop);
    int status = -ENOMEM;
    if (request->parent && request->name && request->result->file)
        status = scoop_io_work_submit(&request->work, loop, id, execute, finish);
    if (status < 0) {
        free(request->parent);
        free(request->name);
        scoop_io_temporary_free(request->result);
        free(request);
    }
    return status;
}
