#include "../../../runtime/include/scoop_rt.h"

#include <stdbool.h>

const ScoopString *native_root_round_trip(const ScoopString *message) {
    void *root = (void *)message;
    void **slots[] = {&root};
    ScoopNativeRootFrame frame;

    scoop_rt_push_native_roots(&frame, slots, 1);
    scoop_rt_gc_collect();

    const ScoopString *reloaded = root;
    bool valid = scoop_rt_gc_debug_native_root_count() == 1 &&
                 reloaded != NULL && reloaded->len == 2 &&
                 reloaded->data[0] == '4' && reloaded->data[1] == '2';

    scoop_rt_pop_native_roots(&frame);
    return valid ? reloaded : NULL;
}
