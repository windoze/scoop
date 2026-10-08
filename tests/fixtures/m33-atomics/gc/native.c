#include "scoop_rt.h"

#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

uint64_t m33_atomic_address(uint64_t handle) {
  return (uint64_t)(uintptr_t)scoop_rt_resolve_handle(handle);
}

int64_t m33_atomic_minor_count(void) {
  ScoopGcMetrics metrics;
  scoop_rt_gc_debug_metrics(&metrics);
  return (int64_t)metrics.minor_collections;
}

bool m33_atomic_moving(void) {
  const char *value = getenv("SCOOP_GC_STRESS_MOVE");
  return value != NULL && strcmp(value, "1") == 0;
}
