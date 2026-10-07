#ifndef SCOOP_RT_THREAD_TESTING_H
#define SCOOP_RT_THREAD_TESTING_H

/* Controlled interleavings and pause measurements are compiled only into the
 * dedicated runtime tests. Production transitions contain no test callbacks. */
#ifdef SCOOP_THREAD_TESTING
typedef enum ScoopThreadTestPoint {
    SCOOP_TEST_NATIVE_SAFE_PUBLISHED,
    SCOOP_TEST_NATIVE_RETURNING_PUBLISHED,
    SCOOP_TEST_NATIVE_RUNNING_OBSERVED,
    SCOOP_TEST_NATIVE_RETURNING_BLOCKED,
    SCOOP_TEST_COLLECTOR_STOPPING,
    SCOOP_TEST_COLLECTOR_WAITING,
    SCOOP_TEST_COLLECTOR_STOPPED,
    SCOOP_TEST_COLLECTOR_RESUMING,
    SCOOP_TEST_NURSERY_RETRY,
} ScoopThreadTestPoint;

void scoop_thread_test_point(ScoopThreadTestPoint point);
#define SCOOP_THREAD_TEST_POINT(point) scoop_thread_test_point(point)
#else
#define SCOOP_THREAD_TEST_POINT(point) ((void)0)
#endif

#endif /* SCOOP_RT_THREAD_TESTING_H */
