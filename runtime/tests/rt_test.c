/* Runtime smoke test entry point.
 *
 * The domain implementations live in runtime/tests/rt_test/. Their call order
 * is part of the deterministic stdout contract in rt_test_expected.txt.
 *
 * Build on Darwin/AArch64 (execution additionally requires compiler-emitted
 * stack-map image metadata):
 *   cc -std=c11 -Wall -Wextra -pthread -fno-omit-frame-pointer \
 *     -fno-optimize-sibling-calls -I runtime/include runtime/src/rt.c \
 *     runtime/src/gc.c runtime/src/gc/allocation.c \
 *     runtime/src/gc/collector.c runtime/src/gc/evacuation.c \
 *     runtime/src/gc/reclamation.c runtime/src/gc/heap.c \
 *     runtime/src/gc/heap_objects.c \
 *     runtime/src/gc/roots.c runtime/src/gc/stackmap.c \
 *     runtime/src/gc/stack_roots.c runtime/src/thread.c \
 *     runtime/src/thread/collection.c runtime/src/thread/debug.c \
 *     runtime/src/thread/roots.c runtime/src/thread/transitions.c \
 *     runtime/src/callback.c runtime/src/platform/profiles/darwin_aarch64.c \
 *     runtime/src/platform/image/macho.c runtime/src/platform/os/darwin.c \
 *     runtime/src/platform/arch/aarch64.c \
 *     runtime/src/platform/arch/aarch64_anchor.S \
 *     runtime/tests/rt_test/support.c \
 *     runtime/tests/rt_test/threads.c \
 *     runtime/tests/rt_test/threads/registration.c \
 *     runtime/tests/rt_test/threads/stw.c \
 *     runtime/tests/rt_test/threads/allocation.c \
 *     runtime/tests/rt_test/threads/transitions.c \
 *     runtime/tests/rt_test/callbacks.c \
 *     runtime/tests/rt_test/core.c runtime/tests/rt_test/gc.c \
 *     runtime/tests/rt_test.c -o /tmp/scoop_rt_test -lc++abi
 */

#include "rt_test/support.h"

void scoop_main(void) {
    run_thread_tests();
    run_callback_tests();
    run_core_runtime_tests();
    run_gc_tests();
}
