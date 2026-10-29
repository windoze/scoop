#ifndef SCOOP_ASYNCIO_TEST_FENCE_WORK_H
#define SCOOP_ASYNCIO_TEST_FENCE_WORK_H

#include "../../native/loop.h"

typedef struct TestWorkFence TestWorkFence;
unsigned scoop_test_pool_size(void);
ScoopIoCreated scoop_test_fence_create(ScoopIoLoop *loop, int64_t id, unsigned spare);
void scoop_test_fence_wait(TestWorkFence *fence);
void scoop_test_fence_release(TestWorkFence *fence);

#endif
