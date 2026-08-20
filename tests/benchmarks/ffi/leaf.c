#include "native.h"

int64_t bench_scalar(int64_t value) { return value + 1; }

BenchPair bench_pair(BenchPair value) { return (BenchPair){value.first + 1, value.second + 2}; }
