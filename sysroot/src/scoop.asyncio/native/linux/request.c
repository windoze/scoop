#include "backend.h"
#include <errno.h>

static void append(RingPending *queue, RingEntry *entry) {
    entry->next = NULL;
    if (queue->tail)
        queue->tail->next = entry;
    else
        queue->head = entry;
    queue->tail = entry;
}

static void remove_pending(RingPending *queue, RingEntry *entry) {
    RingEntry *previous = NULL, *current = queue->head;
    while (current != entry) {
        previous = current;
        current = current->next;
    }
    if (previous)
        previous->next = entry->next;
    else
        queue->head = entry->next;
    if (queue->tail == entry)
        queue->tail = previous;
}

void scoop_io_request_init(ScoopIoRequest *request, ScoopIoLoop *loop, int64_t id) {
    request->loop = loop;
    request->completion.event.id = id;
    request->next = loop->requests;
    loop->requests = request;
    for (unsigned i = 0; i < 6; i++) {
        request->entries[i].request = request;
        request->entries[i].index = i;
    }
}

void scoop_io_request_queue(ScoopIoRequest *request, unsigned index) {
    RingEntry *entry = &request->entries[index];
    entry->state = RING_QUEUED;
    request->live++;
    append(index < 3 ? &request->loop->pending : &request->loop->control, entry);
    scoop_io_loop_wake(request->loop);
}

static void retire(ScoopIoRequest *request) {
    if (!request->ended || request->live || request->retired)
        return;
    request->retired = true;
    ScoopIoRequest **entry = &request->loop->requests;
    while (*entry != request)
        entry = &(*entry)->next;
    *entry = request->next;
    request->finish(request);
}

void scoop_io_request_end(ScoopIoRequest *request) {
    if (request->ended)
        return;
    request->ended = true;
    for (unsigned i = 0; i < 3; i++) {
        RingEntry *entry = &request->entries[i];
        if (entry->state == RING_QUEUED) {
            remove_pending(&request->loop->pending, entry);
            entry->state = RING_FINISHED;
            request->live--;
        } else if (entry->state == RING_PUBLISHED) {
            scoop_io_request_queue(request, i + 3);
        }
    }
    retire(request);
}

void scoop_io_entry_complete(RingEntry *entry, int result) {
    ScoopIoRequest *request = entry->request;
    entry->state = RING_FINISHED;
    request->live--;
    if (entry->index < 3)
        request->result(request, entry->index, result);
    else if (result < 0 && result != -ENOENT && result != -EALREADY && result != -ECANCELED)
        scoop_io_fail(request->loop, -result);
    retire(request);
}

void scoop_io_request_cancel(ScoopIoLoop *loop, int64_t id) {
    for (ScoopIoRequest *request = loop->requests; request; request = request->next) {
        if (request->completion.event.id == id) {
            request->cancelled = true;
            scoop_io_request_end(request);
            return;
        }
    }
}

void scoop_io_request_error(ScoopIoRequest *request, int error) {
    request->completion.event.kind = SCOOP_IO_POSIX_ERROR;
    request->completion.event.code = error;
    scoop_io_request_end(request);
}
