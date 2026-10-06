#ifndef SCOOP_TEST_NURSERY_FIXTURE_H
#define SCOOP_TEST_NURSERY_FIXTURE_H

#include <assert.h>
#include <stddef.h>
#include <stdio.h>
#include <stdlib.h>

#include "../src/gc/gc_internal.h"
#include "../src/gc/heap_internal.h"
#include "../src/managed_entries.h"
#include "../src/thread.h"
#include "platform/image_fixture.h"

typedef struct NurseryNode {
    ScoopObjectHeader header;
    struct NurseryNode *next;
    uint64_t value;
} NurseryNode;

static const uint64_t nursery_node_scan[] = {1, offsetof(NurseryNode, next)};
static const ScoopTypeDescriptor nursery_node_td = {
    .type_id = 801,
    .instance_shape = {.instance_kind = SCOOP_TYPE_INSTANCE_FIXED_OBJECT_V1,
                       .minimum_size = sizeof(NurseryNode),
                       .instance_alignment = 8},
    .object_scan = nursery_node_scan,
};

static void *nursery_allocate(const ScoopTypeDescriptor *td, size_t size) {
    _Alignas(16) uintptr_t frame[4] = {0};
    frame[2] = (uintptr_t)scoop_thread_current_required()->managed_stack_boundary;
    ScoopManagedAnchor anchor;
    scoop_thread_push_managed_anchor(&anchor, 0x1010, (uintptr_t)frame, (uintptr_t)&frame[2]);
    void *object = scoop_gc_alloc_internal(td, size);
    scoop_thread_pop_managed_anchor(&anchor);
    return object;
}

static NurseryNode *nursery_node(uint64_t value) {
    NurseryNode *node = nursery_allocate(&nursery_node_td, sizeof *node);
    node->value = value;
    return node;
}

static void nursery_collect(bool minor) {
    _Alignas(16) uintptr_t frame[4] = {0};
    frame[2] = (uintptr_t)scoop_thread_current_required()->managed_stack_boundary;
    ScoopManagedAnchor anchor;
    scoop_thread_push_managed_anchor(&anchor, 0x1010, (uintptr_t)frame, (uintptr_t)&frame[2]);
    if (minor) {
        scoop_gc_collect_minor_internal();
    } else {
        scoop_gc_collect_internal();
    }
    scoop_thread_pop_managed_anchor(&anchor);
}

static ScoopGcMetrics nursery_metrics(void) {
    ScoopGcMetrics result;
    scoop_rt_gc_debug_metrics(&result);
    return result;
}

#endif
