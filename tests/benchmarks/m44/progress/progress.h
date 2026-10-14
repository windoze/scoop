#ifndef SCOOP_M44_PROGRESS_BENCH_H
#define SCOOP_M44_PROGRESS_BENCH_H

#include <stdatomic.h>
#include <stdbool.h>
#include <stdint.h>

enum { M44_MAX_THREADS = 4, M44_MAX_COLLECTIONS = 10000, M44_PROBE_EVERY = 1024 };

typedef struct M44Worker {
    _Alignas(128) _Atomic(uint64_t) last_ns;
    _Atomic(uint64_t) completed;
    _Atomic(bool) active;
    uint64_t probes;
    int64_t checksum;
} M44Worker;

extern M44Worker m44_workers[M44_MAX_THREADS];
extern _Thread_local unsigned m44_worker_index;
extern unsigned m44_threads;
extern int32_t m44_iterations;

uint64_t m44_now_ns(void);
void m44_observation_init(void);
void m44_report(int mode, uint64_t elapsed, uint64_t allocated_bytes);

#endif
