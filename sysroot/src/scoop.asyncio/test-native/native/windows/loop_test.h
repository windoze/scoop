#ifndef SCOOP_ASYNCIO_WINDOWS_LOOP_TEST_H
#define SCOOP_ASYNCIO_WINDOWS_LOOP_TEST_H

#include "../../../native/windows/backend.h"
#include <errno.h>

#define CHECK(condition)                                                                           \
    do {                                                                                           \
        if (!(condition))                                                                          \
            return __LINE__;                                                                       \
    } while (0)

static inline int test_wait(ScoopIoLoop *loop) {
    return WaitForSingleObject(scoop_io_loop_event(loop), 5000) == WAIT_OBJECT_0 ? 0 : -1;
}

static inline int test_drain(ScoopIoLoop *loop) {
    ULONGLONG deadline = GetTickCount64() + 5000;
    for (;;) {
        if (scoop_io_loop_poll(loop, false).code)
            return -1;
        while (scoop_io_loop_batch_size(loop))
            scoop_io_loop_next(loop);
        if (scoop_io_loop_idle(loop))
            return scoop_io_loop_destroy(loop).code;
        if (GetTickCount64() >= deadline || test_wait(loop))
            return -1;
    }
}

static inline int test_result(ScoopIoLoop *loop, ScoopIoEvent *event) {
    ULONGLONG deadline = GetTickCount64() + 5000;
    for (;;) {
        if (scoop_io_loop_poll(loop, false).code)
            return -1;
        if (scoop_io_loop_batch_size(loop)) {
            *event = scoop_io_loop_next(loop);
            return 0;
        }
        if (GetTickCount64() >= deadline || test_wait(loop))
            return -1;
    }
}

#endif
