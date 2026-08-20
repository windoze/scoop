#include <pthread.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

typedef struct { int32_t left, right; } Pair;

int32_t m23_counter = 30;
const int32_t m23_limit = 32;
Pair m23_pair = {33, 34};
_Thread_local int32_t m23_tls_initial = 31;
_Thread_local int32_t m23_tls_zero;

int32_t m23_increment(int32_t *pointer) { return ++*pointer; }

typedef int32_t (*Callback)(int32_t, void *);
typedef struct {
    pthread_mutex_t mutex;
    pthread_cond_t condition;
    int ready;
    int finished;
} Gate;
typedef struct {
    Gate *gate;
    Callback callback;
    void *context;
    int32_t value;
    int32_t result;
    uintptr_t address;
} Job;

static void rendezvous(Gate *gate, int *count) {
    if (pthread_mutex_lock(&gate->mutex) != 0) abort();
    ++*count;
    if (pthread_cond_broadcast(&gate->condition) != 0) abort();
    while (*count < 2) {
        if (pthread_cond_wait(&gate->condition, &gate->mutex) != 0) abort();
    }
    if (pthread_mutex_unlock(&gate->mutex) != 0) abort();
}

static void *worker(void *argument) {
    Job *job = argument;
    job->address = (uintptr_t)&m23_tls_initial;
    rendezvous(job->gate, &job->gate->ready);
    job->result = job->callback(job->value, job->context);
    if (m23_tls_initial != job->value || m23_tls_zero != job->value + 100 ||
        m23_counter != 70) job->result = -2000;
    rendezvous(job->gate, &job->gate->finished);
    return NULL;
}

int32_t m23_storage_threads(Callback callback, void *context, uint64_t caller) {
    Gate gate = {.ready = 0, .finished = 0};
    if (pthread_mutex_init(&gate.mutex, NULL) != 0 ||
        pthread_cond_init(&gate.condition, NULL) != 0) abort();
    Job jobs[2] = {
        {.gate = &gate, .callback = callback, .context = context, .value = 40},
        {.gate = &gate, .callback = callback, .context = context, .value = 41}
    };
    pthread_t threads[2];
    for (int i = 0; i < 2; ++i) {
        if (pthread_create(&threads[i], NULL, worker, &jobs[i]) != 0) abort();
    }
    for (int i = 0; i < 2; ++i) {
        if (pthread_join(threads[i], NULL) != 0) abort();
    }
    if (pthread_cond_destroy(&gate.condition) != 0 ||
        pthread_mutex_destroy(&gate.mutex) != 0) abort();
    bool independent = jobs[0].address != jobs[1].address &&
        jobs[0].address != caller && jobs[1].address != caller;
    return independent ? jobs[0].result + jobs[1].result : -3000;
}
