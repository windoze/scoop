#include "scoop_rt.h"
#include <stdlib.h>

int32_t m23_class_gc_collected(void) {
    return getenv("SCOOP_GC_STRESS_MOVE") == NULL ||
           scoop_rt_thread_debug_gc_epoch() > 0;
}
