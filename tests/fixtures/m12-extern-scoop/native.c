#include "../../../runtime/include/scoop_rt.h"

#include <stdbool.h>
#include <signal.h>
#include <stddef.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/types.h>
#include <sys/wait.h>
#include <unistd.h>

extern const ScoopTypeDescriptor scoop_td_String;

typedef struct NativeNode {
    ScoopObjectHeader header;
    struct NativeNode *left;
    struct NativeNode *right;
} NativeNode;

typedef struct ManagedAggregate {
    const ScoopString *value;
    int64_t left;
    int64_t right;
} ManagedAggregate;

_Static_assert(sizeof(ManagedAggregate) == 24,
               "Scoop aggregate fixture size must stay exact");
_Static_assert(_Alignof(ManagedAggregate) == 8,
               "Scoop aggregate fixture alignment must stay exact");
_Static_assert(offsetof(ManagedAggregate, value) == 0 &&
                   offsetof(ManagedAggregate, left) == 8 &&
                   offsetof(ManagedAggregate, right) == 16,
               "Scoop aggregate fixture offsets must stay exact");

static const uint64_t node_refs[] = {2, 16, 24};
static const ScoopTypeDescriptor node_td = {
    .type_id = UINT64_C(0x4e41544956454e44),
    .size = sizeof(NativeNode),
    .align = _Alignof(NativeNode),
    .ref_offsets = node_refs,
    .parent = NULL,
    .vtable = NULL,
    .itables = NULL,
    .itable_count = 0,
    .name = "fixture.NativeNode",
};

static bool stress_move_enabled(void) {
    const char *value = getenv("SCOOP_GC_STRESS_MOVE");
    return value != NULL && strcmp(value, "1") == 0;
}

static bool stale_address_faults(const ScoopString *object) {
    fflush(stdout);
    pid_t child = fork();
    if (child == 0) {
        volatile uint64_t length = object->len;
        (void)length;
        _exit(0);
    }
    if (child < 0) {
        return false;
    }
    int status = 0;
    if (waitpid(child, &status, 0) != child || !WIFSIGNALED(status)) {
        return false;
    }
    int signal = WTERMSIG(status);
    return signal == SIGSEGV || signal == SIGBUS;
}

static bool exact_bytes_are_poisoned(const void *object, size_t size) {
    const unsigned char *bytes = object;
    for (size_t index = 0; index < size; index++) {
        if (bytes[index] != 0xA5) {
            return false;
        }
    }
    return true;
}

/*
 * The Scoop ABI entry below is deliberately not expressed as a C
 * struct-by-value function. On Darwin/AArch64 Scoop passes an indirect result
 * in x8 and materializes this 24-byte byval argument at the incoming stack
 * pointer; the assembly shim maps those storage locations onto this ordinary C
 * helper's x0/x1 parameters. This keeps the fixture independent of Clang's C
 * aggregate classifier.
 */
void native_aggregate_round_trip_storage(ManagedAggregate *result,
                                         ManagedAggregate *value) {
    uintptr_t old_value_address = (uintptr_t)value->value;
    int64_t left = value->left;
    int64_t right = value->right;
    void *root = (void *)value->value;
    void **slots[] = {&root};
    ScoopNativeRootFrame frame;

    /* Decompose the value completely before the first GC and retain only its
     * managed leaf in a DirectSlots frame. The aggregate place itself does not
     * cross the GC, so a RecursiveRegion frame is neither required nor used. */
    scoop_rt_push_native_roots(&frame, slots, 1);
    scoop_runtime_gc_collect();
    const ScoopString *reloaded = root;
    bool valid = reloaded != NULL && (uintptr_t)reloaded != old_value_address &&
                 reloaded->len == 9 &&
                 memcmp(reloaded->data, "aggregate", 9) == 0;

    result->value = reloaded;
    result->left = left + 1;
    result->right = right + 2;
    uintptr_t old_result_value_address = (uintptr_t)result->value;
    scoop_runtime_gc_collect();
    reloaded = root;
    valid = valid && reloaded != NULL &&
            (uintptr_t)reloaded != old_result_value_address &&
            result->value == reloaded && reloaded->len == 9 &&
            memcmp(reloaded->data, "aggregate", 9) == 0;
    scoop_rt_pop_native_roots(&frame);

    if (!valid) {
        result->left = -1;
        result->right = -1;
    }
}

__asm__(
    ".text\n"
    ".globl _native_aggregate_round_trip\n"
    ".p2align 2\n"
    "_native_aggregate_round_trip:\n"
    "mov x1, sp\n"
    "mov x0, x8\n"
    "b _native_aggregate_round_trip_storage\n");

const ScoopString *native_root_round_trip(const ScoopString *message) {
    void *root = (void *)message;
    void *large_root = NULL;
    void *node_root = NULL;
    void **slots[] = {&root, &large_root, &node_root};
    ScoopNativeRootFrame frame;

    scoop_rt_push_native_roots(&frame, slots, 3);
    scoop_runtime_gc_collect();

    const ScoopString *reloaded = root;
    bool valid = scoop_rt_gc_debug_native_root_count() == 3 &&
                 scoop_rt_gc_debug_last_moved_count() > 0 &&
                 reloaded != message &&
                 reloaded != NULL &&
                 scoop_rt_gc_debug_allocation_size(reloaded) == 32 &&
                 reloaded->len == 2 &&
                 reloaded->data[0] == '4' && reloaded->data[1] == '2';
    if (stress_move_enabled()) {
        valid = valid && stale_address_faults(message);
    }

    ScoopString *large = scoop_rt_alloc(&scoop_td_String, 224);
    large->len = 200;
    memset(large->data, 'x', 200);
    large_root = large;
    const void *pinned_address = scoop_rt_pin(root);
    const void *old_large_address = large_root;
    scoop_runtime_gc_collect();

    valid = valid && root == pinned_address &&
            large_root != old_large_address &&
            scoop_rt_gc_debug_last_moved_count() > 0 &&
            scoop_rt_gc_debug_allocation_size(large_root) == 224 &&
            ((const ScoopString *)large_root)->len == 200 &&
            ((const ScoopString *)large_root)->data[199] == 'x';
    scoop_rt_unpin(root);

    large_root = NULL;
    NativeNode *child = scoop_rt_alloc(&node_td, sizeof(NativeNode));
    node_root = child;
    NativeNode *parent = scoop_rt_alloc(&node_td, sizeof(NativeNode));
    child = node_root;
    child->left = child;
    child->right = NULL;
    parent->left = child;
    parent->right = child;
    node_root = parent;
    const void *old_parent = parent;
    const void *old_child = child;
    scoop_runtime_gc_collect();

    parent = node_root;
    child = parent->left;
    valid = valid && parent != old_parent && child != old_child &&
            parent->left == parent->right && child->left == child &&
            child->right == NULL &&
            scoop_rt_gc_debug_allocation_size(parent) ==
                sizeof(NativeNode);

    scoop_rt_pin(parent);
    old_child = child;
    scoop_runtime_gc_collect();
    parent = node_root;
    child = parent->left;
    valid = valid && child != old_child && parent->left == parent->right &&
            child->left == child;
    if (stress_move_enabled()) {
        valid = valid &&
                exact_bytes_are_poisoned(old_child, sizeof(NativeNode));
    }
    scoop_rt_unpin(parent);

    /* The first collection quarantined `message`'s old, now-empty block.
     * Later stress allocations and collections must never make it readable
     * or eligible for arena reuse again. */
    if (stress_move_enabled()) {
        valid = valid && stale_address_faults(message);
    }

    scoop_rt_pop_native_roots(&frame);
    return valid ? root : NULL;
}
