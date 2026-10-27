#include "backend.h"

void scoop_io_action_begin(ScoopIoLoop *loop) {
    pthread_mutex_lock(&loop->mutex);
    loop->actions++;
    pthread_mutex_unlock(&loop->mutex);
}

void scoop_io_action_end(ScoopIoLoop *loop) {
    pthread_mutex_lock(&loop->mutex);
    loop->actions--;
    scoop_io_notify_locked(loop);
    pthread_mutex_unlock(&loop->mutex);
}

void scoop_io_request_submit(ScoopIoRequest *request, ScoopIoLoop *loop, int64_t id,
                             dispatch_function_t start, void (*cancel)(ScoopIoRequest *)) {
    request->loop = loop;
    request->completion.event.id = id;
    request->cancel = cancel;
    pthread_mutex_lock(&loop->mutex);
    request->next = loop->requests;
    loop->requests = request;
    loop->actions++;
    pthread_mutex_unlock(&loop->mutex);
    dispatch_async_f(loop->queue, request, start);
}

bool scoop_io_request_start(ScoopIoRequest *request) {
    pthread_mutex_lock(&request->loop->mutex);
    request->started = true;
    bool cancelled = request->cancelled;
    request->cancel_sent = cancelled;
    pthread_mutex_unlock(&request->loop->mutex);
    return !cancelled;
}

void scoop_io_request_finish(ScoopIoRequest *request) {
    ScoopIoLoop *loop = request->loop;
    pthread_mutex_lock(&loop->mutex);
    ScoopIoRequest **entry = &loop->requests;
    while (*entry != request)
        entry = &(*entry)->next;
    *entry = request->next;
    pthread_mutex_unlock(&loop->mutex);
    scoop_io_complete(loop, &request->completion, 0);
}

static void cancel_requests(void *context) {
    ScoopIoLoop *loop = context;
    for (;;) {
        pthread_mutex_lock(&loop->mutex);
        ScoopIoRequest *request = loop->requests;
        while (request && (!request->started || !request->cancelled || request->cancel_sent))
            request = request->next;
        if (request)
            request->cancel_sent = true;
        pthread_mutex_unlock(&loop->mutex);
        if (!request)
            break;
        request->cancel(request);
    }
    scoop_io_action_end(loop);
}

void scoop_io_request_cancel(ScoopIoLoop *loop, int64_t id) {
    pthread_mutex_lock(&loop->mutex);
    for (ScoopIoRequest *request = loop->requests; request; request = request->next) {
        if (request->completion.event.id == id) {
            request->cancelled = true;
            break;
        }
    }
    loop->actions++;
    pthread_mutex_unlock(&loop->mutex);
    dispatch_async_f(loop->queue, loop, cancel_requests);
}
