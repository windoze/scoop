#ifdef _WIN32
#include "../../../scoop.platform/native/windows_api.h"
#define probe_lock(probe) AcquireSRWLockExclusive(&(probe)->mutex)
#define probe_unlock(probe) ReleaseSRWLockExclusive(&(probe)->mutex)
#define probe_wait(probe) SleepConditionVariableSRW(&(probe)->changed, &(probe)->mutex, INFINITE, 0)
#define probe_notify(probe) WakeAllConditionVariable(&(probe)->changed)
#else
#include <pthread.h>
#define probe_lock(probe) pthread_mutex_lock(&(probe)->mutex)
#define probe_unlock(probe) pthread_mutex_unlock(&(probe)->mutex)
#define probe_wait(probe) pthread_cond_wait(&(probe)->changed, &(probe)->mutex)
#define probe_notify(probe) pthread_cond_broadcast(&(probe)->changed)
#endif
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

typedef struct {
#ifdef _WIN32
    SRWLOCK mutex;
    CONDITION_VARIABLE changed;
    DWORD caller;
#else
    pthread_mutex_t mutex;
    pthread_cond_t changed;
    pthread_t caller;
#endif
    int started, finished, released, foreign;
    bool allowed;
} TestWorkProbe;

typedef struct {
    TestWorkProbe *probe;
    int result;
} TestWork;

/**
 * Allocates a probe recording worker-thread execution and resource-release counts; allowed
 * controls its initial gate.
 */
void *scoop_test_native_probe_new(bool allowed) {
    TestWorkProbe *probe = calloc(1, sizeof(*probe));
    if (!probe)
        abort();
#ifdef _WIN32
    probe->caller = GetCurrentThreadId();
#else
    if (pthread_mutex_init(&probe->mutex, NULL) != 0 ||
        pthread_cond_init(&probe->changed, NULL) != 0)
        abort();
    probe->caller = pthread_self();
#endif
    probe->allowed = allowed;
    return probe;
}

/**
 * Destroys the probe after all associated native work and release callbacks have completed.
 */
void scoop_test_native_probe_free(TestWorkProbe *probe) {
#ifndef _WIN32
    pthread_cond_destroy(&probe->changed);
    pthread_mutex_destroy(&probe->mutex);
#endif
    free(probe);
}

/**
 * Allocates work borrowing this probe until its release callback runs.
 */
void *scoop_test_native_work_new(TestWorkProbe *probe) {
    TestWork *work = calloc(1, sizeof(*work));
    if (!work)
        abort();
    work->probe = probe;
    return work;
}

static void execute(void *value) {
    TestWork *work = value;
    TestWorkProbe *probe = work->probe;
    probe_lock(probe);
    probe->started++;
#ifdef _WIN32
    probe->foreign = probe->caller != GetCurrentThreadId();
#else
    probe->foreign = !pthread_equal(probe->caller, pthread_self());
#endif
    probe_notify(probe);
    while (!probe->allowed)
        probe_wait(probe);
    work->result = 42;
    probe->finished++;
    probe_unlock(probe);
}

static void release(void *value) {
    TestWork *work = value;
    probe_lock(work->probe);
    work->probe->released++;
    probe_unlock(work->probe);
    free(work);
}

typedef void (*WorkFunction)(void *);
/**
 * Returns the worker callback that waits on the gate and produces the value 42.
 */
WorkFunction scoop_test_native_execute(void) { return execute; }
/**
 * Returns the cleanup callback that counts release and frees a work allocation.
 */
WorkFunction scoop_test_native_release(void) { return release; }
/**
 * Reads the completed work result before its release callback frees the allocation.
 */
int scoop_test_native_result(TestWork *work) { return work->result; }

/**
 * Blocks until a worker has entered the probe, allowing deterministic cancellation tests.
 */
void scoop_test_native_wait(TestWorkProbe *probe) {
    probe_lock(probe);
    while (!probe->started)
        probe_wait(probe);
    probe_unlock(probe);
}

/**
 * Opens the worker gate and wakes waiting operations.
 */
void scoop_test_native_allow(TestWorkProbe *probe) {
    probe_lock(probe);
    probe->allowed = true;
    probe_notify(probe);
    probe_unlock(probe);
}

/**
 * Reads a synchronized counter: 0 started, 1 finished, 2 released, otherwise whether
 * execution used another thread.
 */
int scoop_test_native_count(TestWorkProbe *probe, int kind) {
    probe_lock(probe);
    int value = kind == 0   ? probe->started
                : kind == 1 ? probe->finished
                : kind == 2 ? probe->released
                            : probe->foreign;
    probe_unlock(probe);
    return value;
}
