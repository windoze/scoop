#include "../../../../runtime/include/scoop_rt.h"
#include "threading.h"

#include <stdatomic.h>
#include <stdint.h>
#include <stdlib.h>

typedef int32_t (*M13Callback)(int32_t value, void *context);
typedef void (*M13ContextFirstCallback)(void *context, int32_t value);
typedef void (*M13PointerCallback)(void *context);

typedef struct M13Gate {
    int32_t expected;
    int32_t arrived;
    uint64_t generation;
    M13Mutex mutex;
    M13Condition condition;
} M13Gate;

static void m13_gate_init(M13Gate *gate, int32_t expected) {
    if (expected <= 0 || m13_mutex_init(&gate->mutex) != 0 ||
        m13_condition_init(&gate->condition) != 0) {
        abort();
    }
    gate->expected = expected;
    gate->arrived = 0;
    gate->generation = 0;
}

static void m13_gate_wait(M13Gate *gate) {
    if (m13_mutex_lock(&gate->mutex) != 0) {
        abort();
    }
    uint64_t generation = gate->generation;
    gate->arrived += 1;
    if (gate->arrived == gate->expected) {
        gate->arrived = 0;
        gate->generation += 1;
        if (m13_condition_broadcast(&gate->condition) != 0) {
            abort();
        }
    } else {
        while (generation == gate->generation) {
            if (m13_condition_wait(&gate->condition, &gate->mutex) != 0) {
                abort();
            }
        }
    }
    if (m13_mutex_unlock(&gate->mutex) != 0) {
        abort();
    }
}

static void m13_gate_destroy(M13Gate *gate) {
    if (m13_condition_destroy(&gate->condition) != 0 || m13_mutex_destroy(&gate->mutex) != 0) {
        abort();
    }
}

static _Atomic(M13Gate *) m13_active_callback_gate;

typedef struct M13Job {
    M13Callback callback;
    void *context;
    int32_t value;
    int32_t result;
    int go;
    M13Thread thread;
    M13Mutex mutex;
    M13Condition condition;
    M13Gate *entry_gate;
} M13Job;

typedef struct M13Group {
    int32_t count;
    M13Job **jobs;
    M13Gate entry_gate;
} M13Group;

static M13ThreadResult M13_THREAD_CALL m13_worker(void *raw_job) {
    M13Job *job = (M13Job *)raw_job;
    m13_mutex_lock(&job->mutex);
    while (!job->go) {
        m13_condition_wait(&job->condition, &job->mutex);
    }
    m13_mutex_unlock(&job->mutex);
    if (job->entry_gate != NULL) {
        m13_gate_wait(job->entry_gate);
    }
    job->result = job->callback(job->value, job->context);
    return 0;
}

static M13Job *m13_create_job(M13Callback callback, void *context, int32_t value,
                              M13Gate *entry_gate) {
    M13Job *job = (M13Job *)calloc(1, sizeof(M13Job));
    if (job == NULL) {
        abort();
    }
    job->callback = callback;
    job->context = context;
    job->value = value;
    job->entry_gate = entry_gate;
    m13_mutex_init(&job->mutex);
    m13_condition_init(&job->condition);
    if (m13_thread_create(&job->thread, m13_worker, job) != 0) {
        abort();
    }
    return job;
}

void *m13_start_one(M13Callback callback, void *context, int32_t value) {
    return m13_create_job(callback, context, value, NULL);
}

int32_t m13_join_one(void *raw_job) {
    M13Job *job = (M13Job *)raw_job;
    m13_mutex_lock(&job->mutex);
    job->go = 1;
    m13_condition_signal(&job->condition);
    m13_mutex_unlock(&job->mutex);
    m13_thread_join(job->thread);
    int32_t result = job->result;
    m13_condition_destroy(&job->condition);
    m13_mutex_destroy(&job->mutex);
    free(job);
    return result;
}

int32_t m13_run_many(M13Callback callback, void *context, int32_t base, int32_t count) {
    if (count <= 0) {
        abort();
    }
    M13Job **jobs = (M13Job **)calloc((size_t)count, sizeof(M13Job *));
    if (jobs == NULL) {
        abort();
    }
    M13Gate entry_gate;
    M13Gate callback_gate;
    m13_gate_init(&entry_gate, count);
    m13_gate_init(&callback_gate, count);
    atomic_store_explicit(&m13_active_callback_gate, &callback_gate, memory_order_release);
    for (int32_t index = 0; index < count; index += 1) {
        jobs[index] = m13_create_job(callback, context, base + index, &entry_gate);
    }
    for (int32_t index = 0; index < count; index += 1) {
        m13_mutex_lock(&jobs[index]->mutex);
        jobs[index]->go = 1;
        m13_condition_signal(&jobs[index]->condition);
        m13_mutex_unlock(&jobs[index]->mutex);
    }
    int32_t result = 0;
    for (int32_t index = 0; index < count; index += 1) {
        m13_thread_join(jobs[index]->thread);
        result += jobs[index]->result;
        m13_condition_destroy(&jobs[index]->condition);
        m13_mutex_destroy(&jobs[index]->mutex);
        free(jobs[index]);
    }
    atomic_store_explicit(&m13_active_callback_gate, NULL, memory_order_release);
    m13_gate_destroy(&callback_gate);
    m13_gate_destroy(&entry_gate);
    free(jobs);
    return result;
}

void m13_callback_barrier(void) {
    M13Gate *gate = atomic_load_explicit(&m13_active_callback_gate, memory_order_acquire);
    if (gate == NULL) {
        abort();
    }
    m13_gate_wait(gate);
}

int32_t m13_attached_thread_count(void) { return (int32_t)scoop_rt_thread_debug_count(); }

int32_t m13_fail_start(M13Callback callback, void *context) {
    (void)callback;
    (void)context;
    return -1;
}

void *m13_start_many(M13Callback callback, void *context, int32_t base, int32_t count) {
    if (count <= 0) {
        abort();
    }
    M13Group *group = (M13Group *)calloc(1, sizeof(M13Group));
    if (group == NULL) {
        abort();
    }
    group->count = count;
    group->jobs = (M13Job **)calloc((size_t)count, sizeof(M13Job *));
    if (group->jobs == NULL) {
        abort();
    }
    m13_gate_init(&group->entry_gate, count);
    for (int32_t index = 0; index < count; index += 1) {
        group->jobs[index] = m13_create_job(callback, context, base + index, &group->entry_gate);
    }
    return group;
}

int32_t m13_join_many(void *raw_group) {
    M13Group *group = (M13Group *)raw_group;
    for (int32_t index = 0; index < group->count; index += 1) {
        m13_mutex_lock(&group->jobs[index]->mutex);
        group->jobs[index]->go = 1;
        m13_condition_signal(&group->jobs[index]->condition);
        m13_mutex_unlock(&group->jobs[index]->mutex);
    }
    int32_t result = 0;
    for (int32_t index = 0; index < group->count; index += 1) {
        M13Job *job = group->jobs[index];
        m13_thread_join(job->thread);
        result += job->result;
        m13_condition_destroy(&job->condition);
        m13_mutex_destroy(&job->mutex);
        free(job);
    }
    m13_gate_destroy(&group->entry_gate);
    free(group->jobs);
    free(group);
    return result;
}

void m13_call_context_first(M13ContextFirstCallback callback, void *context, int32_t value) {
    callback(context, value);
}

typedef struct M13PointerJob {
    M13PointerCallback callback;
    void *context;
    _Atomic bool completed;
} M13PointerJob;

static M13ThreadResult M13_THREAD_CALL m13_pointer_worker(void *raw_job) {
    M13PointerJob *job = (M13PointerJob *)raw_job;
    job->callback(job->context);
    atomic_store_explicit(&job->completed, true, memory_order_release);
    return 0;
}

const ScoopString *m13_borrowed_round_trip(const ScoopString *message, M13PointerCallback callback,
                                           void *context) {
    void *root = (void *)message;
    void **slots[] = {&root};
    ScoopNativeRootFrame frame;
    scoop_rt_push_native_roots(&frame, slots, 1);

    M13PointerJob job = {.callback = callback, .context = context, .completed = false};
    uint64_t epoch = scoop_rt_thread_debug_gc_epoch();
    M13Thread thread;
    if (m13_thread_create(&thread, m13_pointer_worker, &job) != 0) {
        abort();
    }
    while (scoop_rt_thread_debug_gc_epoch() == epoch) {
        m13_thread_yield();
    }

    /* This Scoop-ABI function is executing in native-borrowed mode. The
     * callback's collector cannot proceed until this explicit runtime call
     * parks the original thread and publishes the native root above. */
    do {
        scoop_runtime_gc_collect();
        m13_thread_yield();
    } while (!atomic_load_explicit(&job.completed, memory_order_acquire));
    /* Stress allocation and the callback body can request distinct GC epochs.
     * Stay cooperative until the callback has detached before blocking in join. */
    if (m13_thread_join(thread) != 0) {
        abort();
    }
    const ScoopString *reloaded = (const ScoopString *)root;
    bool valid = scoop_rt_gc_debug_is_allocated(reloaded);
    scoop_rt_pop_native_roots(&frame);
    return valid ? reloaded : NULL;
}
