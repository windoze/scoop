#define _POSIX_C_SOURCE 200809L
#include "../primitives.h"
#include <errno.h>
#include <stdlib.h>
#include <time.h>

int scoop_platform_mutex_init(ScoopPlatformMutex *mutex) { return pthread_mutex_init(mutex, NULL); }
int scoop_platform_mutex_lock(ScoopPlatformMutex *mutex) { return pthread_mutex_lock(mutex); }
int scoop_platform_mutex_unlock(ScoopPlatformMutex *mutex) { return pthread_mutex_unlock(mutex); }
void scoop_platform_mutex_destroy(ScoopPlatformMutex *mutex) { pthread_mutex_destroy(mutex); }
int scoop_platform_condition_wait(ScoopPlatformCondition *condition, ScoopPlatformMutex *mutex) {
    return pthread_cond_wait(condition, mutex);
}
int scoop_platform_condition_wait_for(ScoopPlatformCondition *condition, ScoopPlatformMutex *mutex,
                                      uint32_t microseconds) {
    struct timespec deadline;
    if (clock_gettime(CLOCK_REALTIME, &deadline) != 0)
        return errno;
    uint64_t nanos = (uint64_t)deadline.tv_nsec + (uint64_t)microseconds * 1000;
    deadline.tv_sec += (time_t)(nanos / UINT64_C(1000000000));
    deadline.tv_nsec = (long)(nanos % UINT64_C(1000000000));
    return pthread_cond_timedwait(condition, mutex, &deadline);
}
int scoop_platform_condition_signal(ScoopPlatformCondition *condition) {
    return pthread_cond_signal(condition);
}
int scoop_platform_condition_broadcast(ScoopPlatformCondition *condition) {
    return pthread_cond_broadcast(condition);
}
int scoop_platform_once(ScoopPlatformOnce *once, void (*initialize)(void)) {
    return pthread_once(once, initialize);
}
int scoop_platform_thread_create(ScoopPlatformThread *thread, void *(*entry)(void *),
                                 void *argument) {
    return pthread_create(thread, NULL, entry, argument);
}
int scoop_platform_thread_join(ScoopPlatformThread thread) { return pthread_join(thread, NULL); }
void *scoop_platform_aligned_allocate(size_t alignment, size_t size) {
    void *pointer = NULL;
    return posix_memalign(&pointer, alignment, size) == 0 ? pointer : NULL;
}
void scoop_platform_aligned_free(void *pointer) { free(pointer); }
static bool clock_ns(clockid_t clock, uint64_t *value) {
    struct timespec time;
    if (clock_gettime(clock, &time) != 0)
        return false;
    *value = (uint64_t)time.tv_sec * UINT64_C(1000000000) + (uint64_t)time.tv_nsec;
    return true;
}
bool scoop_platform_monotonic_ns(uint64_t *value) { return clock_ns(CLOCK_MONOTONIC, value); }
bool scoop_platform_thread_cpu_ns(uint64_t *value) {
    return clock_ns(CLOCK_THREAD_CPUTIME_ID, value);
}
