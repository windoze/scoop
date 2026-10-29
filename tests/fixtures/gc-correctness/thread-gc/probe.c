#define _GNU_SOURCE 1
#include "sync_result.h"
#include <assert.h>
#include <errno.h>
#include <pthread.h>
#include <stdatomic.h>
#include <stdint.h>
#include <stdlib.h>

static _Atomic int counts[4], failure;
static pthread_mutex_t gate = PTHREAD_MUTEX_INITIALIZER;
static pthread_cond_t changed = PTHREAD_COND_INITIALIZER;
static int started, proceed, finished;
static void *last_mutex;

struct Launch {
    void *(*start)(void *);
    void *argument;
};

static void *run(void *argument) {
    struct Launch launch = *(struct Launch *)argument;
    free(argument);
    void *result = launch.start(launch.argument);
    assert(pthread_mutex_lock(&gate) == 0);
    finished = 1;
    assert(pthread_cond_broadcast(&changed) == 0);
    assert(pthread_mutex_unlock(&gate) == 0);
    return result;
}

void m45_sync_fail(int kind) { atomic_store(&failure, kind); }
int m45_sync_count(int kind) { return atomic_load(&counts[kind]); }

int m45_thread_create(pthread_t *thread, const pthread_attr_t *attributes, void *(*start)(void *),
                      void *argument) {
    if (atomic_exchange(&failure, 0) == 1)
        return EAGAIN;
    struct Launch *launch = malloc(sizeof(*launch));
    assert(launch != NULL);
    *launch = (struct Launch){start, argument};
    int error = pthread_create(thread, attributes, run, launch);
    if (error != 0)
        free(launch);
    return error;
}

int m45_thread_detach(pthread_t thread) {
    atomic_fetch_add(&counts[1], 1);
    return pthread_detach(thread);
}

void m45_thread_free(void *pointer) {
    atomic_fetch_add(&counts[0], 1);
    free(pointer);
}

void m45_gate_block(void) {
    assert(pthread_mutex_lock(&gate) == 0);
    started = 1;
    assert(pthread_cond_broadcast(&changed) == 0);
    while (!proceed)
        assert(pthread_cond_wait(&changed, &gate) == 0);
    assert(pthread_mutex_unlock(&gate) == 0);
}

void m45_gate_wait_started(void) {
    assert(pthread_mutex_lock(&gate) == 0);
    while (!started)
        assert(pthread_cond_wait(&changed, &gate) == 0);
    assert(pthread_mutex_unlock(&gate) == 0);
}

void m45_gate_finish(void) {
    assert(pthread_mutex_lock(&gate) == 0);
    proceed = 1;
    assert(pthread_cond_broadcast(&changed) == 0);
    while (!finished)
        assert(pthread_cond_wait(&changed, &gate) == 0);
    assert(pthread_mutex_unlock(&gate) == 0);
}

void *m45_mutex_malloc(size_t size) { return last_mutex = malloc(size); }
int m45_mutex_init(pthread_mutex_t *mutex, const pthread_mutexattr_t *attributes) {
    return atomic_exchange(&failure, 0) == 2 ? EAGAIN : pthread_mutex_init(mutex, attributes);
}
void m45_mutex_free(void *pointer) {
    atomic_fetch_add(&counts[2], 1);
    free(pointer);
}

int m45_condition_init(pthread_cond_t *condition, const pthread_condattr_t *attributes) {
    return atomic_exchange(&failure, 0) == 3 ? EAGAIN : pthread_cond_init(condition, attributes);
}
void m45_condition_free(void *pointer) {
    atomic_fetch_add(&counts[3], 1);
    free(pointer);
}

/* Only the probe keeps this raw handle, to clean up its intentionally abandoned lock. */
extern struct ScoopPlatformSyncResult scoop_platform_mutex_unlock(void *);
extern struct ScoopPlatformSyncResult scoop_platform_mutex_destroy(void *);
void m45_cleanup_held_mutex(void) {
    assert(scoop_platform_mutex_unlock(last_mutex).status == 0);
    assert(scoop_platform_mutex_destroy(last_mutex).status == 0);
}
