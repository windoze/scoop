/* Keep the test assertions active when the runtime is built with NDEBUG. */
#undef NDEBUG
#include <signal.h>
#include <sys/wait.h>
#include <unistd.h>

#include "nursery_fixture.h"

static const ScoopTypeDescriptor leaf_td = {
    .type_id = 802,
    .instance_shape = {.instance_kind = SCOOP_TYPE_INSTANCE_FIXED_OBJECT_V1,
                       .minimum_size = sizeof(NurseryNode),
                       .instance_alignment = 8},
};
static const NurseryNode immortal = {
    .header = {.td = &leaf_td},
    .value = 42,
};
static void *root;

static void expect_invalid_reference(const void *address) {
    for (unsigned minor = 0; minor < 2; minor++) {
        pid_t child = fork();
        assert(child >= 0);
        if (child == 0) {
            (void)close(STDERR_FILENO);
            /* Confirm the valid start before its invalid alias. Pinning also keeps
             * relocation from becoming a later, accidental rejection of the slot.
             * A minor must validate even the old target it will not mark. */
            assert(scoop_rt_pin(root) == root);
            void *values[] = {root, (void *)address};
            void **slots[] = {&values[0], &values[1]};
            ScoopNativeRootFrame roots;
            scoop_rt_push_native_roots(&roots, slots, 2);
            nursery_collect(minor != 0);
            _exit(0);
        }
        int status = 0;
        assert(waitpid(child, &status, 0) == child);
        assert(WIFSIGNALED(status) && WTERMSIG(status) == SIGABRT);
    }
}

/* Synthetic managed frames must stay below main's entry boundary at -O2. */
__attribute__((noinline)) static void check_references(void) {
    NurseryNode *before = nursery_node(7);
    before->next = (NurseryNode *)&immortal;
    root = before;
    ScoopGcMetrics metrics = nursery_metrics();
    nursery_collect(true);
    NurseryNode *current = root;
    assert(current != before && current->value == 7);
    assert(current->next == &immortal && current->next->value == 42);
    assert(nursery_metrics().promoted_bytes > metrics.promoted_bytes);
    nursery_collect(false);
    assert(((NurseryNode *)root)->next == &immortal);

    /* Heap membership must not turn invalid object starts into stable references. */
    expect_invalid_reference((const char *)root + 1);
    expect_invalid_reference((const char *)root + sizeof(ScoopObjectHeader));
#if SCOOP_ENABLE_GC_STRESS || (defined(SCOOP_GC_VERIFY_HEAP) && SCOOP_GC_VERIFY_HEAP)
    const NurseryNode unregistered = {.header = {.td = &leaf_td}, .value = 99};
    expect_invalid_reference(&unregistered);
#endif
    root = (void *)&immortal;
    nursery_collect(true);
    nursery_collect(false);
    assert(root == &immortal && immortal.value == 42);
    root = NULL;
}

int main(void) {
    assert(setenv("SCOOP_GC_WORKERS", "1", 1) == 0);
    static const ScoopTypeDescriptor *const types[] = {&nursery_node_td, &leaf_td};
    static const ScoopImmortalObjectDescriptorV1 registration = {
        .object_start = &immortal,
        .object_size = sizeof immortal,
    };
    static const ScoopImmortalObjectDescriptorV1 *const immortals[] = {&registration};
    uintptr_t boundary = 0;
    scoop_thread_runtime_init();
    scoop_test_image_init(types, 2, NULL, 0, immortals, 1);
    scoop_thread_attach_main();
    scoop_thread_enter_managed(&boundary);
    scoop_rt_gc_add_root(&root);
    check_references();
    scoop_thread_leave_managed();
    scoop_thread_detach_main();
    puts("stable references and heap object starts passed");
}
