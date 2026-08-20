#include <assert.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

#include "../src/gc/gc_internal.h"
#include "../src/thread.h"
#include "platform/image_fixture.h"

typedef struct VisitState {
    void **expected_slot;
    void *replacement;
    uint64_t visits;
} VisitState;

static void replace_root(void **slot, void *raw_state) {
    VisitState *state = raw_state;
    assert(slot == state->expected_slot);
    *slot = state->replacement;
    state->visits++;
}

int main(void) {
    ScoopStackMapIndex index = scoop_test_stackmaps();
    scoop_gc_stackmaps_init(&index);

    _Alignas(16) uintptr_t stack[16] = {0};
    void *original = (void *)(uintptr_t)0x1230;
    void *replacement = (void *)(uintptr_t)0x4560;
    memcpy(&stack[0], &original, sizeof original);

    uintptr_t boundary = (uintptr_t)&stack[8];
    uintptr_t frame_record[2] = {boundary, 0};
    memcpy(&stack[2], frame_record, sizeof frame_record);
    ScoopManagedAnchor anchor = {
        .return_pc = 0x1010,
        .stack_pointer = (uintptr_t)&stack[0],
        .frame_pointer = (uintptr_t)&stack[2],
        .previous = NULL,
    };
    ScoopThreadState thread = {
        .stack_low = (const char *)&stack[0],
        .stack_high = (const char *)&stack[16],
        .managed_stack_boundary = (const char *)boundary,
        .managed_anchor = &anchor,
    };
    VisitState state = {
        .expected_slot = (void **)&stack[0],
        .replacement = replacement,
        .visits = 0,
    };
    ScoopGcRootVisitor visitor = {
        .visit_slot = replace_root,
        .visit_external_object = NULL,
        .visit_region = NULL,
        .context = &state,
    };
    scoop_gc_visit_managed_stack(&thread, visitor);

    void *updated = NULL;
    memcpy(&updated, &stack[0], sizeof updated);
    assert(state.visits == 1);
    assert(updated == replacement);

    memcpy(&stack[0], &original, sizeof original);
    state.visits = 0;
    scoop_gc_visit_managed_segment(&thread, anchor.return_pc, anchor.stack_pointer,
                                   anchor.frame_pointer, boundary, visitor);
    memcpy(&updated, &stack[0], sizeof updated);
    assert(state.visits == 1);
    assert(updated == replacement);

    scoop_stackmap_dispose_index(&index);
    puts("exact stack root walk passed");
    return 0;
}
