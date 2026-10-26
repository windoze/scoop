#include "sigpipe.h"
#include "uv_loop.h"

static void wake_callback(uv_async_t *handle) { (void)handle; }

ScoopIoCreated scoop_io_loop_create(void) {
    ScoopIoLoop *loop = calloc(1, sizeof(*loop));
    if (!loop)
        return (ScoopIoCreated){NULL, UV_ENOMEM};
    int status = uv_loop_init(&loop->uv);
    if (status < 0) {
        free(loop);
        return (ScoopIoCreated){NULL, status};
    }
    status = uv_async_init(&loop->uv, &loop->wake, wake_callback);
    if (status < 0) {
        uv_loop_close(&loop->uv);
        free(loop);
        return (ScoopIoCreated){NULL, status};
    }
    return (ScoopIoCreated){loop, 0};
}

void scoop_io_loop_wake(ScoopIoLoop *loop) { uv_async_send(&loop->wake); }

int scoop_io_loop_poll(ScoopIoLoop *loop, bool wait) {
    ScoopIoSignalGuard guard;
    scoop_io_signal_enter(&guard);
    uv_run(&loop->uv, wait && !loop->head ? UV_RUN_ONCE : UV_RUN_NOWAIT);
    scoop_io_signal_leave(&guard);
    return 0;
}

void scoop_io_complete(ScoopIoLoop *loop, ScoopIoCompletion *completion, int status) {
    if (status < 0) {
        completion->event.kind = SCOOP_IO_UV_ERROR;
        completion->event.code = status;
    }
    completion->next = NULL;
    if (loop->tail)
        loop->tail->next = completion;
    else
        loop->head = completion;
    loop->tail = completion;
    loop->completions++;
}

int64_t scoop_io_loop_batch_size(ScoopIoLoop *loop) { return loop->completions; }

ScoopIoEvent scoop_io_loop_next(ScoopIoLoop *loop) {
    ScoopIoCompletion *entry = loop->head;
    if (!entry)
        return (ScoopIoEvent){0};
    loop->head = entry->next;
    loop->completions--;
    if (!loop->head)
        loop->tail = NULL;
    ScoopIoEvent event = entry->event;
    free(entry);
    return event;
}

int scoop_io_loop_destroy(ScoopIoLoop *loop) {
    if (loop->head || loop->timers || loop->connections || loop->work)
        return UV_EBUSY;
    uv_close((uv_handle_t *)&loop->wake, NULL);
    uv_run(&loop->uv, UV_RUN_DEFAULT);
    int status = uv_loop_close(&loop->uv);
    if (status == 0)
        free(loop);
    return status;
}

bool scoop_io_loop_idle(ScoopIoLoop *loop) {
    return !loop->head && !loop->timers && !loop->connections && !loop->work;
}

ScoopIoEvent scoop_io_status_error(int status) {
    return (ScoopIoEvent){.kind = SCOOP_IO_UV_ERROR, .code = status};
}

int scoop_io_loop_descriptor(ScoopIoLoop *loop) { return uv_backend_fd(&loop->uv); }
