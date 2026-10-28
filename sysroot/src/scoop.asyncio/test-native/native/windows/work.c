#include "loop_test.h"

typedef struct {
    ScoopIoWork work;
    HANDLE entered, proceed;
    volatile LONG *executed;
} Work;

static void execute(ScoopIoWork *native) {
    Work *work = (void *)native;
    InterlockedIncrement(work->executed);
    ReleaseSemaphore(work->entered, 1, NULL);
    if (WaitForSingleObject(work->proceed, 5000) != WAIT_OBJECT_0)
        work->work.status = -ETIMEDOUT;
}

static void finish(ScoopIoWork *work, int status) { scoop_io_work_finished(work, status); }

static int submit(ScoopIoLoop *loop, int64_t id, HANDLE entered, HANDLE proceed,
                  volatile LONG *count) {
    Work *work = calloc(1, sizeof(*work));
    if (!work)
        return -ENOMEM;
    work->entered = entered;
    work->proceed = proceed;
    work->executed = count;
    int status = scoop_io_work_submit(&work->work, loop, id, execute, finish);
    if (status)
        free(work);
    return status;
}

int scoop_test_iocp_work(void) {
    ScoopIoCreated created = scoop_io_loop_create();
    CHECK(!created.status.code);
    ScoopIoLoop *loop = created.pointer;
    unsigned width = scoop_io_work_width();
    HANDLE entered = CreateSemaphoreW(NULL, 0, (LONG)width, NULL);
    HANDLE proceed = CreateEventW(NULL, TRUE, FALSE, NULL);
    CHECK(entered && proceed);
    volatile LONG executed = 0;
    for (unsigned i = 1; i <= width; ++i)
        CHECK(!submit(loop, i, entered, proceed, &executed));
    for (unsigned i = 0; i < width; ++i)
        CHECK(WaitForSingleObject(entered, 5000) == WAIT_OBJECT_0);
    CHECK(!submit(loop, 100, entered, proceed, &executed));
    scoop_io_work_cancel(loop, 100);
    ScoopIoEvent event;
    CHECK(!test_result(loop, &event));
    CHECK(event.id == 100 && event.kind == SCOOP_IO_POSIX_ERROR && event.code == ECANCELED);
    CHECK(InterlockedCompareExchange(&executed, 0, 0) == (LONG)width);
    scoop_io_work_cancel(loop, 1);
    CHECK(!scoop_io_loop_poll(loop, false).code && !scoop_io_loop_batch_size(loop));
    CHECK(!scoop_io_loop_idle(loop));
    CHECK(SetEvent(proceed));
    unsigned seen = 0;
    for (unsigned i = 0; i < width; ++i) {
        CHECK(!test_result(loop, &event));
        CHECK(event.kind == SCOOP_IO_DONE && event.id >= 1 && event.id <= width);
        unsigned bit = 1u << (unsigned)(event.id - 1);
        CHECK(!(seen & bit));
        seen |= bit;
    }
    CHECK(InterlockedCompareExchange(&executed, 0, 0) == (LONG)width);
    CHECK(!test_drain(loop));
    CHECK(CloseHandle(entered) && CloseHandle(proceed));
    return 0;
}
