#ifndef SCOOP_ASYNCIO_WORK_H
#define SCOOP_ASYNCIO_WORK_H

#include "loop.h"

struct ScoopIoWork {
    ScoopIoCompletion completion;
    uv_work_t request;
    ScoopIoLoop *loop;
    ScoopIoWork *next;
};

int scoop_io_work_submit(ScoopIoWork *work, ScoopIoLoop *loop, int64_t id, uv_work_cb execute,
                         uv_after_work_cb finished);
void scoop_io_work_finished(ScoopIoWork *work, int status);
void scoop_io_work_cancel(ScoopIoLoop *loop, int64_t id);

#endif
