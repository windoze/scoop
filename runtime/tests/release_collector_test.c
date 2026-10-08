#include "no_core.h"
#include <assert.h>
#include <signal.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/resource.h>
#include <sys/wait.h>
#include <unistd.h>

#include "../src/gc/gc_internal.h"
#include "../src/gc/heap_internal.h"
#include "../src/managed_entries.h"
#include "../src/thread.h"
#include "platform/image_fixture.h"

#define OTHER_HEADER_BIT UINT64_C(1)
#define PAYLOAD_MARKER UINT64_C(0x1255aabbfedc9876)

typedef struct Resource {
    ScoopObjectHeader header;
    uint64_t handle;
    uint64_t marker;
    struct Resource *child;
} Resource;

typedef struct LargeResource {
    Resource resource;
    uint8_t bytes[5000];
} LargeResource;

static unsigned attempts[16], freed[16];
static const uint64_t child_scan[] = {1, offsetof(Resource, child)};

static void release_resource(void *object) {
    Resource *resource = object;
    assert(resource->header.gc_word == OTHER_HEADER_BIT);
    assert(resource->marker == PAYLOAD_MARKER);
    assert(resource->handle < 16);
    if (resource->header.td->instance_shape.minimum_size == sizeof(LargeResource)) {
        const LargeResource *large = object;
        assert(large->bytes[0] == 0x12);
        assert(large->bytes[sizeof large->bytes - 1] == 0x34);
    }
    attempts[resource->handle]++;
    if (resource->handle != 0) {
        freed[resource->handle]++;
        assert(freed[resource->handle] == 1);
    }
}

static const ScoopTypeDescriptor small_td = {
    .type_id = 1,
    .instance_shape = {.instance_kind = SCOOP_TYPE_INSTANCE_FIXED_OBJECT_V1,
                       .minimum_size = sizeof(Resource),
                       .instance_alignment = _Alignof(Resource)},
    .object_scan = child_scan,
    .release_hook = release_resource,
};
static const ScoopTypeDescriptor large_td = {
    .type_id = 2,
    .instance_shape = {.instance_kind = SCOOP_TYPE_INSTANCE_FIXED_OBJECT_V1,
                       .minimum_size = sizeof(LargeResource),
                       .instance_alignment = _Alignof(LargeResource)},
    .object_scan = child_scan,
    .release_hook = release_resource,
};
static ScoopTypeDescriptor no_hook_td;

static Resource *allocate(const ScoopTypeDescriptor *td, uint64_t handle, bool ready) {
    _Alignas(16) uintptr_t frame[4] = {0};
    frame[2] = (uintptr_t)scoop_thread_current_required()->managed_stack_boundary;
    ScoopManagedAnchor anchor;
    scoop_thread_push_managed_anchor(&anchor, 0x1010, (uintptr_t)frame, (uintptr_t)&frame[2]);
    Resource *resource = scoop_gc_alloc_internal(td, td->instance_shape.minimum_size);
    scoop_thread_pop_managed_anchor(&anchor);
    assert(resource->header.gc_word == 0);
    resource->handle = handle;
    resource->marker = PAYLOAD_MARKER;
    resource->header.gc_word = OTHER_HEADER_BIT;
    if (td->instance_shape.minimum_size == sizeof(LargeResource)) {
        LargeResource *large = (LargeResource *)resource;
        large->bytes[0] = 0x12;
        large->bytes[sizeof large->bytes - 1] = 0x34;
    }
    if (ready) {
        (void)__atomic_fetch_or(&resource->header.gc_word, GC_RELEASE_READY_BIT, __ATOMIC_RELEASE);
    }
    return resource;
}

static Resource *collect(Resource *root) {
    _Alignas(16) uintptr_t frame[4] = {0};
    memcpy(&frame[0], &root, sizeof root);
    frame[2] = (uintptr_t)scoop_thread_current_required()->managed_stack_boundary;
    scoop_rt_gc_collect_impl(0x1010, (uintptr_t)&frame[0], (uintptr_t)&frame[2]);
    memcpy(&root, &frame[0], sizeof root);
    return root;
}

static void lifecycle(const ScoopTypeDescriptor *td) {
    (void)allocate(&no_hook_td, 15, false);
    (void)collect(NULL);
    assert(attempts[15] == 0);

    Resource *unfinished = allocate(td, 2, false);
    unfinished = collect(unfinished);
    assert(unfinished->header.gc_word == OTHER_HEADER_BIT);
    (void)collect(NULL);
    assert(attempts[2] == 0);

    Resource *live = allocate(td, 1, true);
    for (unsigned index = 0; index < 3; index++) {
        uintptr_t old = (uintptr_t)live;
        live = collect(live);
        assert((uintptr_t)live != old);
        assert(live->header.gc_word == (OTHER_HEADER_BIT | GC_RELEASE_READY_BIT));
        assert(scoop_rt_gc_debug_last_moved_count() == 1);
        assert(attempts[1] == 0);
    }
    (void)collect(NULL);
    (void)collect(NULL);
    assert(attempts[1] == 1 && freed[1] == 1);

    (void)allocate(td, 4, true);
    (void)collect(NULL);
    assert(attempts[4] == 1);

    Resource *closed = allocate(td, 12, true);
    closed->handle = 0;
    freed[12]++;
    (void)collect(NULL);
    assert(attempts[0] == 1 && freed[0] == 0 && freed[12] == 1);

    Resource *lazy = allocate(td, 0, true);
    lazy = collect(lazy);
    lazy->handle = 11;
    (void)collect(NULL);
    assert(attempts[11] == 1);
}

static void roots_and_cycles(const ScoopTypeDescriptor *td, bool stress) {
    Resource *pinned = allocate(td, 5, true);
    assert(scoop_rt_pin(pinned) == pinned);
    static void *live;
    scoop_rt_gc_add_root(&live);
    live = allocate(td, 6, true);
    Resource *dead = allocate(td, 7, true);
    if (!stress && td == &small_td) {
        assert((uintptr_t)pinned / GC_BLOCK_SIZE == (uintptr_t)dead / GC_BLOCK_SIZE);
    }
    uintptr_t old_live = (uintptr_t)live;
    (void)collect(NULL);
    assert(attempts[5] == 0 && attempts[6] == 0 && attempts[7] == 1);
    assert((uintptr_t)live != old_live);
    assert((pinned->header.gc_word & UINT64_C(7)) ==
           (OTHER_HEADER_BIT | GC_PIN_BIT | GC_RELEASE_READY_BIT));
    assert(scoop_rt_unpin(pinned) == pinned);
    assert(pinned->header.gc_word == (OTHER_HEADER_BIT | GC_RELEASE_READY_BIT));
    live = NULL;
    (void)collect(NULL);
    assert(attempts[5] == 1 && attempts[6] == 1);

    static void *first;
    scoop_rt_gc_add_root(&first);
    first = allocate(td, 9, true);
    Resource *second = allocate(td, 10, true);
    ((Resource *)first)->child = second;
    second->child = first;
    (void)collect(NULL);
    assert(attempts[9] == 0 && attempts[10] == 0);
    first = NULL;
    (void)collect(NULL);
    assert(attempts[9] == 1 && attempts[10] == 1);
    (void)collect(NULL);
}

static void run(bool large, bool stress, bool bad_header) {
    if (stress) {
        assert(setenv("SCOOP_GC_STRESS_MOVE", "1", 1) == 0);
    } else {
        assert(unsetenv("SCOOP_GC_STRESS_MOVE") == 0);
    }
    const ScoopTypeDescriptor *td = large ? &large_td : &small_td;
    no_hook_td = *td;
    no_hook_td.type_id = 3;
    no_hook_td.release_hook = NULL;
    const ScoopTypeDescriptor *types[] = {&small_td, &large_td, &no_hook_td};
    uintptr_t boundary = 0;
    scoop_thread_runtime_init();
    scoop_test_image_init(types, 3, NULL, 0, NULL, 0);
    scoop_thread_attach_main();
    scoop_thread_enter_managed(&boundary);
    if (bad_header) {
        (void)allocate(&no_hook_td, 15, true);
        (void)collect(NULL);
        abort();
    }
    lifecycle(td);
    roots_and_cycles(td, stress);
    Resource *at_shutdown = allocate(td, 8, true);
    at_shutdown = collect(at_shutdown);
    assert(at_shutdown->header.gc_word & GC_RELEASE_READY_BIT);
    scoop_thread_leave_managed();
    scoop_thread_prepare_shutdown();
    scoop_thread_detach_main();
    scoop_thread_runtime_finish_shutdown();
    assert(attempts[8] == 0);
}

static void child_case(bool large, bool stress, bool bad_header) {
    int output[2];
    assert(pipe(output) == 0);
    pid_t child = fork();
    assert(child >= 0);
    if (child == 0) {
        close(output[0]);
        assert(dup2(output[1], STDERR_FILENO) >= 0);
        close(output[1]);
        scoop_test_disable_core_dumps();
        run(large, stress, bad_header);
        _exit(0);
    }
    close(output[1]);
    char message[2048] = {0};
    ssize_t size = read(output[0], message, sizeof message - 1);
    assert(size >= 0);
    close(output[0]);
    int status;
    assert(waitpid(child, &status, 0) == child);
    if (bad_header) {
        assert(WIFSIGNALED(status) && WTERMSIG(status) == SIGABRT);
        assert(strstr(message, "release-ready object has no release hook") != NULL);
    } else if (!WIFEXITED(status) || WEXITSTATUS(status) != 0) {
        fprintf(stderr, "large=%d stress=%d status=%d: %s", large, stress, status, message);
        abort();
    }
}

int main(void) {
    scoop_test_disable_core_dumps();
    for (unsigned large = 0; large < 2; large++) {
        for (unsigned stress = 0; stress < 2; stress++) {
            child_case(large, stress, false);
            child_case(large, stress, true);
        }
    }
    puts("release collector lifecycle tests passed");
}
