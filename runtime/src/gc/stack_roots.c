#include <inttypes.h>
#include <stdio.h>
#include <stdlib.h>

#include "../platform/platform.h"
#include "../thread.h"
#include "gc_internal.h"

static const ScoopStackMapIndex *stackmaps;
static const ScoopPlatformBundle *platform_bundle;

static _Noreturn void stack_roots_fatal(const char *message) {
    fprintf(stderr, "scoop stackmap: %s\n", message);
    abort();
}

void scoop_gc_stackmaps_init(const ScoopStackMapIndex *index) {
    if (stackmaps != NULL || index == NULL) {
        stack_roots_fatal("invalid stack map index publication");
    }
    platform_bundle = scoop_platform_bundle();
    stackmaps = index;
}

const ScoopStackMapRecord *scoop_gc_stackmap_lookup(uintptr_t return_pc) {
    if (stackmaps == NULL) {
        stack_roots_fatal("stack map lookup before initialization");
    }
    return scoop_stackmap_lookup(stackmaps, return_pc);
}

static _Noreturn void managed_frame_fatal(ScoopPlatformError error) {
    fprintf(stderr, "scoop stackmap: SafepointId %" PRIu64 " root %u: %s\n",
            error.safepoint_id, (unsigned)error.root_index,
            scoop_platform_error_message(error.code));
    abort();
}

static void visit_managed_segment(const ScoopThreadState *thread,
                                  ScoopManagedAnchor cursor, uintptr_t boundary,
                                  ScoopGcRootVisitor visitor) {
    if (stackmaps == NULL || platform_bundle == NULL) {
        stack_roots_fatal("managed stack visit before platform initialization");
    }
    if (thread == NULL || visitor.visit_slot == NULL || cursor.return_pc == 0 ||
        cursor.stack_pointer == 0 || cursor.frame_pointer == 0 ||
        cursor.previous != NULL || boundary == 0) {
        stack_roots_fatal("managed segment has no exact anchor publication");
    }

    ScoopPlatformStackBounds bounds = {
        .low = thread->stack_low,
        .high = thread->stack_high,
    };

    for (;;) {
        const ScoopStackMapRecord *record = scoop_gc_stackmap_lookup(cursor.return_pc);
        if (record == NULL) {
            fprintf(stderr,
                    "scoop stackmap: no exact record for managed return PC "
                    "0x%" PRIxPTR "\n",
                    cursor.return_pc);
            abort();
        }

        ScoopManagedFrame frame;
        ScoopPlatformError error = {0};
        if (!platform_bundle->managed_frames->frame_from_anchor(&cursor, record, bounds,
                                                                &frame, &error)) {
            managed_frame_fatal(error);
        }
        for (uint16_t index = 0; index < record->root_count; index++) {
            void **slot = NULL;
            error = (ScoopPlatformError){0};
            if (!platform_bundle->managed_frames->resolve_root(&frame, index, &slot,
                                                               &error) ||
                slot == NULL) {
                managed_frame_fatal(error);
            }
            visitor.visit_slot(slot, visitor.context);
        }

        uintptr_t return_pc = 0;
        uintptr_t stack_pointer = 0;
        uintptr_t frame_pointer = 0;
        bool has_next = false;
        error = (ScoopPlatformError){0};
        if (!platform_bundle->managed_frames->next_frame(
                &frame, boundary, bounds, &return_pc, &stack_pointer, &frame_pointer,
                &has_next, &error)) {
            managed_frame_fatal(error);
        }
        if (!has_next) {
            return;
        }
        cursor = (ScoopManagedAnchor){
            .return_pc = return_pc,
            .stack_pointer = stack_pointer,
            .frame_pointer = frame_pointer,
            .previous = NULL,
        };
    }
}

void scoop_gc_visit_managed_stack(const ScoopThreadState *thread,
                                  ScoopGcRootVisitor visitor) {
    if (thread == NULL || thread->managed_anchor == NULL ||
        thread->managed_anchor->previous != NULL ||
        thread->managed_stack_boundary == NULL) {
        stack_roots_fatal("managed thread has no exact top-frame publication");
    }
    ScoopManagedAnchor cursor = *thread->managed_anchor;
    cursor.previous = NULL;
    visit_managed_segment(thread, cursor, (uintptr_t)thread->managed_stack_boundary,
                          visitor);
}

void scoop_gc_visit_managed_segment(const ScoopThreadState *thread, uintptr_t return_pc,
                                    uintptr_t stack_pointer, uintptr_t frame_pointer,
                                    uintptr_t managed_boundary,
                                    ScoopGcRootVisitor visitor) {
    ScoopManagedAnchor cursor = {
        .return_pc = return_pc,
        .stack_pointer = stack_pointer,
        .frame_pointer = frame_pointer,
        .previous = NULL,
    };
    visit_managed_segment(thread, cursor, managed_boundary, visitor);
}
