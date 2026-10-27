#include "../../native/linux/backend.h"
#include <errno.h>
#include <poll.h>
#include <unistd.h>

static int wait_cq(ScoopIoLoop *loop, unsigned wanted) {
    while (io_uring_cq_ready(&loop->ring) < wanted) {
        uint64_t hint;
        ssize_t consumed = read(loop->notification, &hint, sizeof(hint));
        (void)consumed;
        if (io_uring_cq_ready(&loop->ring) >= wanted)
            break;
        struct pollfd notification = {.fd = loop->notification, .events = POLLIN};
        int status;
        do {
            status = poll(&notification, 1, 5000);
        } while (status < 0 && errno == EINTR);
        if (status <= 0)
            return status < 0 ? -errno : -ETIMEDOUT;
    }
    return 0;
}
static void drain(ScoopIoLoop *loop) {
    for (ScoopIoRequest *request = loop->requests; request;) {
        ScoopIoRequest *next = request->next;
        scoop_io_request_cancel(loop, request->completion.event.id);
        request = next;
    }
    while (!scoop_io_loop_idle(loop)) {
        scoop_io_loop_poll(loop, true);
        while (scoop_io_loop_batch_size(loop))
            scoop_io_loop_next(loop);
    }
    scoop_io_loop_destroy(loop);
}

int scoop_test_ring_cancel_order(bool control_first) {
    ScoopIoCreated created = scoop_io_loop_create();
    if (created.status < 0)
        return created.status;
    ScoopIoLoop *loop = created.pointer;
    int status = scoop_io_timer_start(loop, 42, 1000, 0);
    if (status < 0) {
        drain(loop);
        return status;
    }
    scoop_io_ring_poll(loop);
    scoop_io_timer_cancel(loop, 42);
    scoop_io_ring_poll(loop);
    status = wait_cq(loop, 2);
    if (!status) {
        struct io_uring_cqe *cqes[2];
        if (io_uring_peek_batch_cqe(&loop->ring, cqes, 2) != 2)
            status = -EPROTO;
        else {
            RingEntry *entries[2] = {io_uring_cqe_get_data(cqes[0]),
                                     io_uring_cqe_get_data(cqes[1])};
            int results[2] = {cqes[0]->res, cqes[1]->res};
            io_uring_cq_advance(&loop->ring, 2);
            unsigned first = (entries[0]->index >= 3) == control_first ? 0 : 1;
            scoop_io_entry_complete(entries[first], results[first]);
            if (scoop_io_loop_batch_size(loop))
                status = -EPROTO;
            scoop_io_entry_complete(entries[1 - first], results[1 - first]);
            if (scoop_io_loop_batch_size(loop) != 1 || scoop_io_loop_next(loop).id != 42)
                status = -EPROTO;
        }
    }
    drain(loop);
    return status;
}

int scoop_test_ring_pressure(void) {
    ScoopIoCreated created = scoop_io_loop_create();
    if (created.status < 0)
        return created.status;
    ScoopIoLoop *loop = created.pointer;
    unsigned capacity = loop->ring.sq.ring_entries;
    int status = 0;
    for (unsigned i = 0; i < capacity * 3; i++) {
        status = scoop_io_timer_start(loop, i + 1, 1000, 0);
        if (status < 0) {
            drain(loop);
            return status;
        }
    }
    scoop_io_ring_poll(loop);
    unsigned queued = 0, published = 0;
    for (ScoopIoRequest *request = loop->requests; request; request = request->next) {
        if (request->entries[0].state == RING_QUEUED)
            queued++;
        if (request->entries[0].state == RING_PUBLISHED)
            published++;
    }
    if (!queued || !published)
        status = -EPROTO;
    for (unsigned i = 0; i < capacity * 3; i++)
        scoop_io_timer_cancel(loop, i + 1);
    unsigned completed = 0;
    while (!scoop_io_loop_idle(loop)) {
        scoop_io_loop_poll(loop, true);
        while (scoop_io_loop_batch_size(loop)) {
            scoop_io_loop_next(loop);
            completed++;
        }
    }
    if (completed != capacity * 3)
        status = -EPROTO;
    scoop_io_loop_destroy(loop);
    return status;
}

int scoop_test_ring_data_ready(ScoopIoLoop *loop) {
    scoop_io_ring_poll(loop);
    return wait_cq(loop, 1);
}

int scoop_test_ring_capabilities(void) {
    ScoopIoCreated created = scoop_io_loop_create();
    if (created.status)
        return created.status;
    ScoopIoLoop *loop = created.pointer;
    struct io_uring_probe *probe = io_uring_get_probe_ring(&loop->ring);
    int status = probe ? 0 : -ENOMEM;
    if (probe) {
        if (scoop_io_ring_capabilities(probe, IORING_FEAT_NODROP) ||
            scoop_io_ring_capabilities(probe, 0) != -EOPNOTSUPP)
            status = -EPROTO;
        probe->ops[IORING_OP_FSYNC].flags = 0;
        if (scoop_io_ring_capabilities(probe, IORING_FEAT_NODROP) != -EOPNOTSUPP)
            status = -EPROTO;
        io_uring_free_probe(probe);
    }
    drain(loop);
    return status;
}
