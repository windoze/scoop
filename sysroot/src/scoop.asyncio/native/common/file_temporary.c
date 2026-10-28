#include "../../../scoop.platform/native/paths.h"
#include "temporary.h"
#include <errno.h>
#include <string.h>

typedef struct {
    ScoopIoWork work;
    ScoopIoTemporaryFile *result;
} TemporaryWork;

static void execute_temporary(ScoopIoWork *native) {
    TemporaryWork *request = (void *)native;
    ScoopIoTemporaryFile *result = request->result;
    result->file->handle = scoop_file_wrap(scoop_platform_file_temporary(result->path));
    int saved_errno = errno;
    ScoopIoEvent *event = &request->work.completion.event;
    if (result->file->handle.value == UINT64_MAX) {
        event->kind = SCOOP_IO_POSIX_ERROR;
        event->code = saved_errno;
    } else {
        event->kind = SCOOP_IO_TEMPORARY;
        event->resource = result;
    }
}

static void finish_temporary(ScoopIoWork *native, int status) {
    TemporaryWork *request = (void *)native;
    if (request->result->file->handle.value == UINT64_MAX)
        scoop_io_temporary_free(request->result);
    scoop_io_work_finished(&request->work, status);
}

int scoop_io_file_temporary(ScoopIoLoop *loop, int64_t id, const char *pattern) {
    TemporaryWork *request = calloc(1, sizeof(*request));
    if (!request)
        return -ENOMEM;
    request->result = calloc(1, sizeof(*request->result));
    if (!request->result) {
        free(request);
        return -ENOMEM;
    }
    ScoopIoTemporaryFile *result = request->result;
    result->file = scoop_io_file_new(loop);
    result->path = strdup(pattern);
    if (!result->file || !result->path) {
        scoop_io_temporary_free(result);
        free(request);
        return -ENOMEM;
    }
    int status =
        scoop_io_work_submit(&request->work, loop, id, execute_temporary, finish_temporary);
    if (status < 0) {
        scoop_io_temporary_free(result);
        free(request);
    }
    return status;
}
