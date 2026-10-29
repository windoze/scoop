#include "../../../scoop.platform/native/paths.h"
#include "platform.h"
#include "work.h"
#include <errno.h>
#include <string.h>

typedef enum { PATH_MOVE, PATH_DELETE, PATH_MKDIR } PathAction;
typedef struct {
    ScoopIoWork work;
    ScoopIoChar *first, *second;
    PathAction action;
    int mode;
    bool replace;
} PathWork;

static void execute_path(ScoopIoWork *native) {
    PathWork *request = (void *)native;
    ScoopPlatformFileResult result;
    switch (request->action) {
    case PATH_MOVE:
        result = scoop_platform_path_move_result(request->first, request->second, request->replace);
        break;
    case PATH_DELETE:
        result = scoop_platform_path_delete_result(request->first);
        break;
    case PATH_MKDIR:
        result = scoop_platform_path_mkdir_result(request->first, request->mode);
        break;
    default:
        abort();
    }
    scoop_io_platform_error(&request->work.completion.event, result.error);
}

static void finish_path(ScoopIoWork *native, int status) {
    PathWork *request = (void *)native;
    free(request->first);
    free(request->second);
    scoop_io_work_finished(&request->work, status);
}

static int submit_path(ScoopIoLoop *loop, int64_t id, PathAction action, const ScoopIoChar *first,
                       const ScoopIoChar *second, int mode, bool replace) {
    PathWork *request = calloc(1, sizeof(*request));
    if (!request)
        return -ENOMEM;
    request->first = scoop_io_string_copy(first);
    request->second = second ? scoop_io_string_copy(second) : NULL;
    if (!request->first || (second && !request->second)) {
        free(request->first);
        free(request->second);
        free(request);
        return -ENOMEM;
    }
    request->action = action;
    request->mode = mode;
    request->replace = replace;
    int status = scoop_io_work_submit(&request->work, loop, id, execute_path, finish_path);
    if (status < 0) {
        free(request->first);
        free(request->second);
        free(request);
    }
    return status;
}

int scoop_io_path_move(ScoopIoLoop *loop, int64_t id, const ScoopIoChar *from,
                       const ScoopIoChar *to, bool replace) {
    return submit_path(loop, id, PATH_MOVE, from, to, 0, replace);
}

int scoop_io_path_delete(ScoopIoLoop *loop, int64_t id, const ScoopIoChar *path) {
    return submit_path(loop, id, PATH_DELETE, path, NULL, 0, false);
}

int scoop_io_path_mkdir(ScoopIoLoop *loop, int64_t id, const ScoopIoChar *path, int mode) {
    return submit_path(loop, id, PATH_MKDIR, path, NULL, mode, false);
}
