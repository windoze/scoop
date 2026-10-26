#include "../../../scoop.platform/native/socket_native.h"
#include "work.h"
#include <errno.h>
#include <string.h>

typedef struct {
    ScoopIoWork work;
    char *host;
    int port, family;
    struct ScoopResolution result;
} ScoopIoResolve;

static void resolve_execute(ScoopIoWork *native) {
    ScoopIoResolve *request = (void *)native;
    request->result = scoop_platform_resolve(request->host, request->port, request->family);
}

static void resolve_finished(ScoopIoWork *native, int status) {
    ScoopIoResolve *request = (void *)native;
    free(request->host);
    ScoopIoEvent *event = &request->work.completion.event;
    if (status == 0 && request->result.code == 0) {
        event->kind = SCOOP_IO_ADDRESSES;
        event->resource = request->result.head;
    } else if (status == 0) {
        event->kind = SCOOP_IO_ADDRESS_ERROR;
        event->code = request->result.code;
        event->count = request->result.has_system_code ? request->result.system_code : -1;
    }
    scoop_io_work_finished(&request->work, status);
}

int scoop_io_resolve(ScoopIoLoop *loop, int64_t id, const char *host, int port, int family) {
    ScoopIoResolve *request = calloc(1, sizeof(*request));
    if (!request)
        return -ENOMEM;
    request->host = strdup(host);
    if (!request->host) {
        free(request);
        return -ENOMEM;
    }
    request->port = port;
    request->family = family;
    int status = scoop_io_work_submit(&request->work, loop, id, resolve_execute, resolve_finished);
    if (status < 0) {
        free(request->host);
        free(request);
    }
    return status;
}
