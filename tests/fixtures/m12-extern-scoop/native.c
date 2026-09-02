#include "../../../runtime/include/scoop_rt.h"

#include <stdbool.h>
#include <string.h>

extern const ScoopTypeDescriptor scoop_td_String;

typedef struct NativeNode {
    ScoopObjectHeader header;
    struct NativeNode *left;
    struct NativeNode *right;
} NativeNode;

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

    scoop_rt_pop_native_roots(&frame);
    return valid ? reloaded : NULL;
}
