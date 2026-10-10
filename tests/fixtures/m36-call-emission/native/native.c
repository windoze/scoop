#include "scoop_rt.h"

#include <assert.h>
#include <stdint.h>

const ScoopString *m36_move(const ScoopString *value) {
    assert(scoop_rt_thread_debug_mode() == SCOOP_THREAD_DEBUG_NATIVE_BORROWED);
    void *root = (void *)value;
    void **slots[] = {&root};
    ScoopNativeRootFrame frame;
    scoop_rt_push_native_roots(&frame, slots, 1);
    scoop_runtime_gc_collect();
    scoop_rt_pop_native_roots(&frame);
    return root;
}

int32_t m36_safe(int32_t value) {
    assert(scoop_rt_thread_debug_mode() == SCOOP_THREAD_DEBUG_NATIVE_SAFE);
    return value + 1;
}

int32_t m36_leaf(int32_t value) {
    assert(scoop_rt_thread_debug_mode() == SCOOP_THREAD_DEBUG_MANAGED);
    return value + 2;
}
