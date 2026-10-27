#include "../loop.h"
#include <dispatch/dispatch.h>
#include <errno.h>
#include <pthread.h>

typedef int (*Notify)(void *);
typedef int (*Observe)(void *, int);
typedef struct {
    dispatch_source_t source;
    Notify ready, removed;
    Observe observe;
    void *ready_context, *removed_context, *observer_context;
} MainWatch;

static void watch_ready(void *context) {
    MainWatch *watch = context;
    watch->ready(watch->ready_context);
    watch->observe(watch->observer_context, 0);
}

static void watch_removed(void *context) {
    MainWatch *watch = context;
    watch->removed(watch->removed_context);
    // The one-shot invocation has returned; its independently retained observer is still alive.
    watch->observe(watch->observer_context, 1);
    dispatch_release(watch->source);
    free(watch);
}

bool scoop_io_main_thread(void) { return pthread_main_np() != 0; }

ScoopIoCreated scoop_io_main_watch(int descriptor, Notify ready, void *ready_context,
                                   Observe observe, void *observer_context) {
    MainWatch *watch = calloc(1, sizeof(*watch));
    if (!watch)
        return (ScoopIoCreated){NULL, -ENOMEM};
    watch->source = dispatch_source_create(DISPATCH_SOURCE_TYPE_READ, (uintptr_t)descriptor, 0,
                                           dispatch_get_main_queue());
    if (!watch->source) {
        free(watch);
        return (ScoopIoCreated){NULL, -ENOMEM};
    }
    watch->ready = ready;
    watch->ready_context = ready_context;
    watch->observe = observe;
    watch->observer_context = observer_context;
    dispatch_set_context(watch->source, watch);
    dispatch_source_set_event_handler_f(watch->source, watch_ready);
    dispatch_source_set_cancel_handler_f(watch->source, watch_removed);
    dispatch_activate(watch->source);
    return (ScoopIoCreated){watch, 0};
}

void scoop_io_main_remove(MainWatch *watch, Notify removed, void *context) {
    watch->removed = removed;
    watch->removed_context = context;
    dispatch_source_cancel(watch->source);
}
