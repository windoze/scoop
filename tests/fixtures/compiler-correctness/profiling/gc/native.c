#include <stdbool.h>

/* Exercise real bounded slices at deterministic points in the managed program. */
void scoop_gc_collect_incremental_internal(bool minor);

void profiling_gc_step(void) { scoop_gc_collect_incremental_internal(false); }
