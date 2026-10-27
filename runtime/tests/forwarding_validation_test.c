#include <signal.h>
#include <string.h>
#include <sys/wait.h>
#include <unistd.h>

#include "nursery_fixture.h"

/* Inject invalid metadata only in child processes at the real forwarding boundary. */
static void corrupt_forwarding(unsigned scenario) {
    const ScoopTypeDescriptor *types[] = {&nursery_node_td};
    scoop_thread_runtime_init();
    scoop_test_image_init(types, 1, NULL, 0, NULL, 0);
    scoop_thread_attach_main();
    scoop_thread_enter_managed((void *)scoop_rt_thread_debug_stack_high());
    void *object = nursery_node(73);
    void **slots[] = {&object};
    ScoopNativeRootFrame roots;
    scoop_rt_push_native_roots(&roots, slots, 1);
    if (scenario == 5) {
        object = (char *)object + sizeof(void *);
    }
    nursery_collect(false);
    assert(nursery_metrics().full_collections == 1);
#if SCOOP_GC_TRACK_RELOCATION
    scoop_gc_heap_lock();
    ScoopGcBlockMeta *block;
    size_t word;
    assert(object_meta(object, &block, &word));
    collection_active = scenario != 0;
    if (scenario != 1) {
        bit_set(block->marks, word);
    }
    block->state = SCOOP_BLOCK_EVACUATION_SOURCE;
    if (scenario == 3) {
        ((ScoopObjectHeader *)object)->gc_word = (uintptr_t)object | GC_FORWARDED_BIT;
    } else if (scenario == 4) {
        ((ScoopObjectHeader *)object)->gc_word = GC_RELEASE_READY_BIT;
    }
    (void)scoop_heap_forward_object(object, block, word);
#endif
    _exit(2);
}

static void expect_failure(unsigned scenario, const char *message) {
    int errors[2];
    assert(pipe(errors) == 0);
    pid_t child = fork();
    assert(child >= 0);
    if (child == 0) {
        close(errors[0]);
        assert(dup2(errors[1], STDERR_FILENO) == STDERR_FILENO);
        close(errors[1]);
        corrupt_forwarding(scenario);
        _exit(3);
    }
    close(errors[1]);
    char output[1024] = {0};
    size_t used = 0;
    ssize_t bytes;
    while ((bytes = read(errors[0], output + used, sizeof output - 1 - used)) > 0) {
        used += (size_t)bytes;
        assert(used < sizeof output - 1);
    }
    assert(bytes == 0);
    close(errors[0]);
    int status;
    assert(waitpid(child, &status, 0) == child);
    assert(WIFSIGNALED(status) && WTERMSIG(status) == SIGABRT);
    assert(strstr(output, message) != NULL);
}

int main(void) {
    expect_failure(5, "managed slot is not a current heap object start");
#if SCOOP_GC_TRACK_RELOCATION
    expect_failure(0, "object forwarded outside collection");
    expect_failure(1, "unmarked object reached relocation");
    expect_failure(2, "marked evacuation source has no planned forwarding address");
    expect_failure(3, "evacuation source forwards to its own address");
    expect_failure(4, "marked evacuation source has no planned forwarding address");
#endif
    puts("mark boundaries and forwarding verification reject invalid references");
    return 0;
}
