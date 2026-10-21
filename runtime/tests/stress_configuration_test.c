#include <assert.h>
#include <stdio.h>
#include <stdlib.h>

#include "../src/gc/gc_internal.h"
#include "../src/gc/heap_internal.h"

int main(void) {
    const char *values[] = {"1", "0", NULL};
    for (size_t index = 0; index < sizeof values / sizeof values[0]; index++) {
        if (values[index] != NULL) {
            assert(setenv("SCOOP_GC_STRESS_MOVE", values[index], 1) == 0);
            assert(setenv("SCOOP_GC_STRESS_MINOR", values[index], 1) == 0);
        } else {
            assert(unsetenv("SCOOP_GC_STRESS_MOVE") == 0);
            assert(unsetenv("SCOOP_GC_STRESS_MINOR") == 0);
        }
        scoop_gc_heap_state.ready = false;
        scoop_gc_heap_init();
        assert(scoop_gc_stress_move_enabled() == (SCOOP_ENABLE_GC_STRESS && index == 0));
#if SCOOP_ENABLE_GC_STRESS
        assert(scoop_gc_heap_state.stress_minor == (index == 0));
#endif
    }
    puts("stress configuration passed");
    return 0;
}
