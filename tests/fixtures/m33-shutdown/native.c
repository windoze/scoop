#include "scoop_rt.h"

#include <pthread.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

typedef int32_t (*Callback)(void *);

static pthread_mutex_t mutex = PTHREAD_MUTEX_INITIALIZER;
static pthread_cond_t condition = PTHREAD_COND_INITIALIZER;
static pthread_t worker;
static bool ready;
static bool finish;
static Callback callback;
static void *callback_context;
static int32_t result;

static void check(int error) {
    if (error != 0) {
        abort();
    }
}

void m33_shutdown_hold(void) {
    check(pthread_mutex_lock(&mutex));
    ready = true;
    check(pthread_cond_broadcast(&condition));
    while (!finish) {
        check(pthread_cond_wait(&condition, &mutex));
    }
    check(pthread_mutex_unlock(&mutex));
}

static void *attached_worker(void *argument) {
    (void)argument;
    if (!scoop_rt_attach_foreign_thread()) {
        abort();
    }
    m33_shutdown_hold();
    scoop_rt_detach_foreign_thread();
    return NULL;
}

static void *callback_worker(void *argument) {
    (void)argument;
    result = callback(callback_context);
    return NULL;
}

static void start(void *(*entry)(void *)) {
    check(pthread_mutex_lock(&mutex));
    ready = false;
    finish = false;
    check(pthread_create(&worker, NULL, entry, NULL));
    while (!ready) {
        check(pthread_cond_wait(&condition, &mutex));
    }
    check(pthread_mutex_unlock(&mutex));
}

void m33_shutdown_start_attached(void) { start(attached_worker); }

void m33_shutdown_start_callback(Callback entry, void *context) {
    callback = entry;
    callback_context = context;
    start(callback_worker);
}

int32_t m33_shutdown_join(void) {
    check(pthread_mutex_lock(&mutex));
    finish = true;
    check(pthread_cond_broadcast(&condition));
    check(pthread_mutex_unlock(&mutex));
    check(pthread_join(worker, NULL));
    return result;
}

int32_t m33_shutdown_exit_requested(void) { return getenv("M33_SHUTDOWN_EXIT") != NULL; }

static void report_atexit(void) { fputs("atexit\n", stdout); }

void m33_shutdown_register_atexit(void) { check(atexit(report_atexit)); }
