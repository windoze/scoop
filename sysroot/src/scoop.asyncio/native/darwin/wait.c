#include "wait.h"
#include <errno.h>
#include <limits.h>

static void source_retired(void *context) {
    ScoopIoWait *wait = context;
    if (++wait->retired != wait->count)
        return;
    for (unsigned i = 0; i < wait->count; i++)
        dispatch_release(wait->sources[i]);
    wait->finish(wait);
}

static int add_source(ScoopIoWait *wait, dispatch_source_t source, dispatch_function_t ready) {
    if (!source)
        return -ENOMEM;
    wait->sources[wait->count++] = source;
    dispatch_set_context(source, wait);
    dispatch_source_set_event_handler_f(source, ready);
    dispatch_source_set_cancel_handler_f(source, source_retired);
    dispatch_activate(source);
    return 0;
}

int scoop_io_wait_fd(ScoopIoWait *wait, int fd, bool writing, dispatch_function_t ready) {
    dispatch_source_t source =
        dispatch_source_create(writing ? DISPATCH_SOURCE_TYPE_WRITE : DISPATCH_SOURCE_TYPE_READ,
                               (uintptr_t)fd, 0, wait->request.loop->queue);
    return add_source(wait, source, ready);
}

int scoop_io_wait_timer(ScoopIoWait *wait, ScoopIoDeadline deadline, dispatch_function_t ready) {
    dispatch_source_t source =
        dispatch_source_create(DISPATCH_SOURCE_TYPE_TIMER, 0, 0, wait->request.loop->queue);
    if (!source)
        return -ENOMEM;
    dispatch_source_set_timer(source,
                              dispatch_time(DISPATCH_TIME_NOW, scoop_io_remaining(deadline)),
                              DISPATCH_TIME_FOREVER, 0);
    return add_source(wait, source, ready);
}

void scoop_io_wait_end(ScoopIoWait *wait) {
    if (wait->ended)
        return;
    wait->ended = true;
    if (!wait->count) {
        wait->finish(wait);
        return;
    }
    for (unsigned i = 0; i < wait->count; i++)
        dispatch_source_cancel(wait->sources[i]);
}

void scoop_io_wait_error(ScoopIoWait *wait, int error) {
    if (wait->ended)
        return;
    wait->request.completion.event.kind = SCOOP_IO_POSIX_ERROR;
    wait->request.completion.event.code = error;
    scoop_io_wait_end(wait);
}
