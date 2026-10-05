#include "../../../runtime/include/scoop_rt.h"

#include <assert.h>
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

_Static_assert(sizeof(NativeNode) == 32 && _Alignof(NativeNode) == 8,
               "Scoop node fixture size and alignment must stay exact");
_Static_assert(offsetof(NativeNode, left) == 16 &&
                   offsetof(NativeNode, right) == 24,
               "Scoop node fixture references must retain their offsets");

static bool stress_move_enabled(void) {
    /* Normal collection selects eligible blocks; only stress collection must
     * relocate every unpinned object (runtime spec 3.7). */
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
 * struct-by-value function. The assembly shim maps Scoop's indirect result and
 * stack byval argument onto this ordinary C helper's two pointer parameters.
 * This keeps the fixture independent of the C aggregate classifier.
 */
void native_aggregate_round_trip_storage(ManagedAggregate *result,
                                         ManagedAggregate *value) {
    bool stress = stress_move_enabled();
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
    bool valid = reloaded != NULL &&
                 (!stress || (uintptr_t)reloaded != old_value_address) &&
                 reloaded->len == 9 &&
                 memcmp(reloaded->data, "aggregate", 9) == 0;
    assert(valid && "native aggregate argument must survive collection");

    result->value = reloaded;
    result->left = left + 1;
    result->right = right + 2;
    uintptr_t old_result_value_address = (uintptr_t)result->value;
    scoop_runtime_gc_collect();
    reloaded = root;
    valid = valid && reloaded != NULL &&
            (!stress || (uintptr_t)reloaded != old_result_value_address) &&
            result->value == reloaded && reloaded->len == 9 &&
            memcmp(reloaded->data, "aggregate", 9) == 0;
    scoop_rt_pop_native_roots(&frame);

    assert(valid && "native aggregate result must survive collection");
}

#if defined(__APPLE__) && defined(__aarch64__)
__asm__(
    ".text\n"
    ".globl _native_aggregate_round_trip\n"
    ".p2align 2\n"
    "_native_aggregate_round_trip:\n"
    "mov x1, sp\n"
    "mov x0, x8\n"
    "b _native_aggregate_round_trip_storage\n");
#elif defined(__linux__) && defined(__x86_64__)
__asm__(
    ".text\n"
    ".globl native_aggregate_round_trip\n"
    ".type native_aggregate_round_trip,@function\n"
    "native_aggregate_round_trip:\n"
    ".cfi_startproc\n"
    "push %rbp\n"
    ".cfi_def_cfa_offset 16\n"
    ".cfi_offset %rbp,-16\n"
    "mov %rsp,%rbp\n"
    ".cfi_def_cfa_register %rbp\n"
    "push %rdi\n"
    "sub $8,%rsp\n"
    "lea 16(%rbp),%rsi\n"
    "call native_aggregate_round_trip_storage\n"
    "mov -8(%rbp),%rax\n"
    "leave\n"
    ".cfi_def_cfa %rsp,8\n"
    "ret\n"
    ".cfi_endproc\n"
    ".size native_aggregate_round_trip,.-native_aggregate_round_trip\n");
#else
#error "Scoop ABI fixture requires a supported target"
#endif

const ScoopString *native_root_round_trip(const ScoopString *message,
                                          const NativeNode *prototype) {
    const ScoopTypeDescriptor *node_td = prototype->header.td;
    bool stress = stress_move_enabled();
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
                 (!stress || reloaded != message) &&
                 reloaded != NULL &&
                 scoop_rt_gc_debug_allocation_size(reloaded) == 32 &&
                 reloaded->len == 2 &&
                 reloaded->data[0] == '4' && reloaded->data[1] == '2';
    if (stress) {
        valid = valid && stale_address_faults(message);
    }
    assert(valid && "native direct roots must survive collection");

    ScoopString *large = scoop_rt_alloc(&scoop_td_String, 224);
    large->len = 200;
    memset(large->data, 'x', 200);
    large_root = large;
    const void *pinned_address = scoop_rt_pin(root);
    const void *old_large_address = large_root;
    scoop_runtime_gc_collect();

    valid = valid && root == pinned_address &&
            (!stress || large_root != old_large_address) &&
            scoop_rt_gc_debug_last_moved_count() > 0 &&
            scoop_rt_gc_debug_allocation_size(large_root) == 224 &&
            ((const ScoopString *)large_root)->len == 200 &&
            ((const ScoopString *)large_root)->data[199] == 'x';
    assert(valid && "pinned roots and large strings must survive collection");
    scoop_rt_unpin(root);

    large_root = NULL;
    NativeNode *child = scoop_rt_alloc(node_td, sizeof(NativeNode));
    node_root = child;
    NativeNode *parent = scoop_rt_alloc(node_td, sizeof(NativeNode));
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
    valid = valid && (!stress || (parent != old_parent && child != old_child)) &&
            parent->left == parent->right && child->left == child &&
            child->right == NULL &&
            scoop_rt_gc_debug_allocation_size(parent) ==
                sizeof(NativeNode);
    assert(valid && "native cycles and shared references must survive collection");

    scoop_rt_pin(parent);
    old_child = child;
    scoop_runtime_gc_collect();
    parent = node_root;
    child = parent->left;
    valid = valid && (!stress || child != old_child) &&
            parent->left == parent->right &&
            child->left == child;
    if (stress) {
        valid = valid &&
                exact_bytes_are_poisoned(old_child, sizeof(NativeNode));
    }
    assert(valid && "pinned objects must retain their relocated child references");
    scoop_rt_unpin(parent);

    /* The first collection quarantined `message`'s old, now-empty block.
     * Later stress allocations and collections must never make it readable
     * or eligible for arena reuse again. */
    if (stress) {
        valid = valid && stale_address_faults(message);
    }
    assert(valid && "stress collections must retain quarantined old blocks");

    scoop_rt_pop_native_roots(&frame);
    return root;
}
