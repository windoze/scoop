#include "backend.h"
#include <errno.h>
#include <sys/eventfd.h>
#include <unistd.h>

int scoop_io_ring_capabilities(const struct io_uring_probe *probe, unsigned features) {
    if (!(features & IORING_FEAT_NODROP))
        return -EOPNOTSUPP;
    const int required[] = {IORING_OP_ACCEPT,  IORING_OP_CONNECT,      IORING_OP_RECV,
                            IORING_OP_SEND,    IORING_OP_RECVMSG,      IORING_OP_SENDMSG,
                            IORING_OP_READ,    IORING_OP_WRITE,        IORING_OP_FSYNC,
                            IORING_OP_TIMEOUT, IORING_OP_ASYNC_CANCEL, IORING_OP_POLL_ADD};
    for (unsigned i = 0; i < sizeof(required) / sizeof(required[0]); i++)
        if (!io_uring_opcode_supported(probe, required[i]))
            return -EOPNOTSUPP;
    return 0;
}

int scoop_io_ring_create(ScoopIoLoop *loop) {
    struct io_uring_params parameters = {0};
    int status = io_uring_queue_init_params(128, &loop->ring, &parameters);
    if (status < 0)
        return status;
    struct io_uring_probe *probe = io_uring_get_probe_ring(&loop->ring);
    if (!probe) {
        status = -errno;
        if (!status)
            status = -ENOMEM;
        goto fail;
    }
    status = scoop_io_ring_capabilities(probe, parameters.features);
    io_uring_free_probe(probe);
    if (status < 0)
        goto fail;
    loop->notification = eventfd(0, EFD_NONBLOCK | EFD_CLOEXEC);
    if (loop->notification < 0) {
        status = -errno;
        goto fail;
    }
    status = io_uring_register_eventfd(&loop->ring, loop->notification);
    if (status < 0) {
        close(loop->notification);
        goto fail;
    }
    return 0;
fail:
    io_uring_queue_exit(&loop->ring);
    return status;
}

static bool retryable(int status) {
    return status == -EINTR || status == -EAGAIN || status == -EBUSY;
}
static void ring_error(ScoopIoLoop *loop, int status) {
    if (status >= 0)
        return;
    if (retryable(status))
        scoop_io_loop_wake(loop);
    else
        scoop_io_fail(loop, -status);
    // Published SQEs remain owned until CQEs arrive. liburing tracks partial
    // submission via the kernel SQ head; never repack or free published entries.
}

void scoop_io_ring_poll(ScoopIoLoop *loop) {
    unsigned count = io_uring_cq_ready(&loop->ring);
    for (unsigned i = 0; i < count; i++) {
        struct io_uring_cqe *completion;
        int status = io_uring_peek_cqe(&loop->ring, &completion);
        if (status < 0) {
            ring_error(loop, status);
            break;
        }
        RingEntry *entry = io_uring_cqe_get_data(completion);
        int result = completion->res;
        io_uring_cqe_seen(&loop->ring, completion);
        scoop_io_entry_complete(entry, result);
    }
    if (io_uring_cq_has_overflow(&loop->ring)) {
        ring_error(loop, io_uring_get_events(&loop->ring));
    }
    while (loop->control.head || loop->pending.head) {
        struct io_uring_sqe *sqe = io_uring_get_sqe(&loop->ring);
        if (!sqe)
            break;
        RingPending *queue = loop->control.head ? &loop->control : &loop->pending;
        RingEntry *entry = queue->head;
        queue->head = entry->next;
        if (!queue->head)
            queue->tail = NULL;
        if (entry->index >= 3)
            io_uring_prep_cancel(sqe, &entry->request->entries[entry->index - 3], 0);
        else
            entry->request->prepare(entry->request, entry->index, sqe);
        io_uring_sqe_set_data(sqe, entry);
        entry->state = RING_PUBLISHED;
    }
    if (io_uring_sq_ready(&loop->ring)) {
        int status = io_uring_submit(&loop->ring);
        loop->submission_blocked = status < 0 && !retryable(status);
        ring_error(loop, status);
    }
    if (io_uring_cq_ready(&loop->ring) ||
        (!loop->submission_blocked &&
         (io_uring_sq_ready(&loop->ring) ||
          ((loop->control.head || loop->pending.head) && io_uring_sq_space_left(&loop->ring)))))
        scoop_io_loop_wake(loop);
}
