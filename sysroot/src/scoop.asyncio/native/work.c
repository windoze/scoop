#include "work.h"

int scoop_io_work_submit(ScoopIoWork *work, ScoopIoLoop *loop, int64_t id, uv_work_cb execute,
                         uv_after_work_cb finished) {
    work->loop = loop;
    work->completion.event.id = id;
    work->request.data = work;
    int status = uv_queue_work(&loop->uv, &work->request, execute, finished);
    if (status < 0)
        return status;
    work->next = loop->work;
    loop->work = work;
    return 0;
}

void scoop_io_work_finished(ScoopIoWork *work, int status) {
    ScoopIoWork **entry = &work->loop->work;
    while (*entry != work)
        entry = &(*entry)->next;
    *entry = work->next;
    scoop_io_complete(work->loop, &work->completion, status);
}

void scoop_io_work_cancel(ScoopIoLoop *loop, int64_t id) {
    for (ScoopIoWork *work = loop->work; work; work = work->next) {
        if (work->completion.event.id == id) {
            (void)uv_cancel((uv_req_t *)&work->request);
            return;
        }
    }
}
