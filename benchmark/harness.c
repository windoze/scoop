#define _POSIX_C_SOURCE 200809L
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <time.h>

int64_t bench_now(void) {
  struct timespec value;
  if (clock_gettime(CLOCK_MONOTONIC, &value) != 0)
    abort();
  return (int64_t)value.tv_sec * 1000000000LL + value.tv_nsec;
}

int64_t bench_config(int32_t key) {
  const char *names[] = {"BENCH_CASE",      "BENCH_N",         "BENCH_SEED",
                         "BENCH_WARMUP_MS", "BENCH_SAMPLE_MS", "BENCH_SAMPLES"};
  const int64_t defaults[] = {0, 1000, 42, 500, 200, 5};
  if (key < 0 || key >= 6)
    abort();
  const char *value = getenv(names[key]);
  return value == NULL ? defaults[key] : strtoll(value, NULL, 10);
}

void bench_check(int64_t seed, int64_t value) {
  printf("{\"type\":\"check\",\"seed\":%lld,\"value\":%lld}\n", (long long)seed,
         (long long)value);
}

void bench_sample(int64_t index, int64_t elapsed, int64_t calls,
                  int64_t checksum) {
  printf("{\"type\":\"sample\",\"index\":%lld,\"elapsed_ns\":%lld,"
         "\"calls\":%lld,\"checksum\":%lld}\n",
         (long long)index, (long long)elapsed, (long long)calls,
         (long long)checksum);
}
