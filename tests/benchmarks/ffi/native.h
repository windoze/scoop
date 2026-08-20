#ifndef SCOOP_FFI_BENCHMARK_H
#define SCOOP_FFI_BENCHMARK_H

#include <stdint.h>

typedef struct BenchPair {
    int64_t first, second;
} BenchPair;

int64_t bench_scalar(int64_t value);
BenchPair bench_pair(BenchPair value);

#endif
