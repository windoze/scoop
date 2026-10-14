#include "scoop_rt.h"

int64_t moved_count(void) { return (int64_t)scoop_rt_gc_debug_last_moved_count(); }
