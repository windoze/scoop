#ifdef _WIN32
#define _CRT_SECURE_NO_WARNINGS
#endif
#include "scoop_rt.h"

#include "../callback/threading.h"
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

typedef int32_t (*Callback)(void *);

static M13Mutex mutex = M13_MUTEX_INITIALIZER;
static M13Condition condition = M13_CONDITION_INITIALIZER;
static M13Thread worker;
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
    check(m13_mutex_lock(&mutex));
    ready = true;
    check(m13_condition_broadcast(&condition));
    while (!finish) {
        check(m13_condition_wait(&condition, &mutex));
    }
    check(m13_mutex_unlock(&mutex));
}

static M13ThreadResult M13_THREAD_CALL attached_worker(void *argument) {
    (void)argument;
    if (!scoop_rt_attach_foreign_thread()) {
        abort();
    }
    m33_shutdown_hold();
    scoop_rt_detach_foreign_thread();
    return 0;
}

static M13ThreadResult M13_THREAD_CALL callback_worker(void *argument) {
    (void)argument;
    result = callback(callback_context);
    return 0;
}

static void start(M13ThreadResult(M13_THREAD_CALL *entry)(void *)) {
    check(m13_mutex_lock(&mutex));
    ready = false;
    finish = false;
    check(m13_thread_create(&worker, entry, NULL));
    while (!ready) {
        check(m13_condition_wait(&condition, &mutex));
    }
    check(m13_mutex_unlock(&mutex));
}

void m33_shutdown_start_attached(void) { start(attached_worker); }

void m33_shutdown_start_callback(Callback entry, void *context) {
    callback = entry;
    callback_context = context;
    start(callback_worker);
}

int32_t m33_shutdown_join(void) {
    check(m13_mutex_lock(&mutex));
    finish = true;
    check(m13_condition_broadcast(&condition));
    check(m13_mutex_unlock(&mutex));
    check(m13_thread_join(worker));
    return result;
}

int32_t m33_shutdown_exit_requested(void) { return getenv("M33_SHUTDOWN_EXIT") != NULL; }

static void report_atexit(void) { fputs("atexit\n", stdout); }

void m33_shutdown_register_atexit(void) { check(atexit(report_atexit)); }
