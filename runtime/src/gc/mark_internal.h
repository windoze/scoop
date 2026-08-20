#ifndef SCOOP_GC_MARK_INTERNAL_H
#define SCOOP_GC_MARK_INTERNAL_H

#include "gc_internal.h"
#include "heap_internal.h"

#define GC_MARK_MAX_WORKERS 8
#define GC_MARK_BATCH 64
#define GC_MARK_CHUNK 1024
#define GC_MARK_ARRAY_RANGE 1024

typedef struct ScoopGcLiveChunk {
    struct ScoopGcLiveChunk *next;
    size_t used;
    void *objects[GC_MARK_CHUNK];
} ScoopGcLiveChunk;

typedef struct ScoopGcMarkTask {
    bool array;
    union {
        struct {
            void *const *objects;
            uint64_t scans;
        } batch;
        struct {
            void *object;
            const uint64_t *scan;
            uintptr_t begin;
            uintptr_t end;
        } range;
    } data;
} ScoopGcMarkTask;

typedef struct ScoopGcMarkQueue {
    pthread_mutex_t lock;
    ScoopGcMarkTask *tasks;
    size_t capacity;
    size_t first;
    size_t count;
} ScoopGcMarkQueue;

typedef struct ScoopGcMarkBlockCount {
    size_t live_bytes;
    size_t movable_bytes;
} ScoopGcMarkBlockCount;

typedef struct ScoopGcMarkWorker {
    _Alignas(128) ScoopGcMarkQueue queue;
    _Alignas(128) size_t id;
    pthread_t thread;
    uint64_t initial_phase;
    bool minor;
    ScoopGcMarkBlockCount *blocks;
    size_t block_capacity;
    ScoopGcLiveChunk *live;
    ScoopGcLiveChunk *tail;
    void *const *pending_objects;
    size_t pending_count;
    uint64_t pending_scans;
    uint64_t marked;
    uint64_t traced;
    uint64_t root_slots;
    uint64_t old_slots;
    uint64_t reference_slots;
    uint64_t tasks;
    uint64_t array_tasks;
    uint64_t stolen_tasks;
    uint64_t cpu_ns;
} ScoopGcMarkWorker;

typedef struct ScoopGcMarker {
    pthread_mutex_t lock;
    pthread_cond_t phase_changed;
    pthread_cond_t work_available;
    pthread_cond_t finished;
    _Atomic(size_t) pending;
    size_t configured_workers;
    size_t active_workers;
    size_t created_workers;
    size_t finished_workers;
    uint64_t phase;
    bool initialized;
    bool forced;
    bool stopping;
    ScoopGcMarkWorker workers[GC_MARK_MAX_WORKERS];
} ScoopGcMarker;

typedef enum ScoopGcMarkSource {
    SCOOP_MARK_ROOT,
    SCOOP_MARK_REMEMBERED,
    SCOOP_MARK_OBJECT,
} ScoopGcMarkSource;

typedef struct ScoopGcMarkVisit {
    ScoopGcMarkWorker *worker;
    ScoopGcMarkSource source;
} ScoopGcMarkVisit;

extern ScoopGcMarker scoop_gc_marker;

void scoop_gc_mark_pool_prepare(bool minor);
void scoop_gc_mark_pool_run(void);
void scoop_gc_mark_notify_work(void);
void scoop_gc_mark_queue_push(ScoopGcMarkWorker *worker, ScoopGcMarkTask task);
bool scoop_gc_mark_queue_take(ScoopGcMarkWorker *worker, ScoopGcMarkTask *task);
void scoop_gc_mark_task_done(void);
void scoop_gc_mark_flush(ScoopGcMarkWorker *worker);
void scoop_gc_mark_slot(void **slot, void *context);
void scoop_gc_mark_run_task(ScoopGcMarkWorker *worker, ScoopGcMarkTask task);

#endif
