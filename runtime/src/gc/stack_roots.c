#include <inttypes.h>
#include <stdio.h>
#include <stdlib.h>

#include "../platform/platform.h"
#include "../thread.h"
#include "gc_internal.h"

static ScoopStackMapIndex stackmaps;
static bool stackmaps_initialized;

static _Noreturn void stack_roots_fatal(const char *message) {
    fprintf(stderr, "scoop stackmap: %s\n", message);
    abort();
}

static void require_complete_bundle(const ScoopPlatformBundle *bundle) {
    if (bundle == NULL || bundle->metadata_images == NULL ||
        bundle->thread_vm == NULL || bundle->managed_frames == NULL ||
        bundle->metadata_images->loaded_images == NULL ||
        bundle->thread_vm->stack_bounds == NULL ||
        bundle->thread_vm->page_size == NULL ||
        bundle->thread_vm->protect_none == NULL ||
        bundle->managed_frames->validate_record == NULL ||
        bundle->managed_frames->frame_from_anchor == NULL ||
        bundle->managed_frames->resolve_root == NULL ||
        bundle->managed_frames->next_frame == NULL) {
        stack_roots_fatal("selected platform bundle is incomplete");
    }
}

void scoop_gc_stackmaps_init(void) {
    if (stackmaps_initialized) {
        stack_roots_fatal("stack maps were initialized more than once");
    }
    const ScoopPlatformBundle *bundle = scoop_platform_bundle();
    require_complete_bundle(bundle);

    ScoopPlatformMetadataImages images;
    ScoopPlatformError platform_error = {0};
    if (!bundle->metadata_images->loaded_images(&images, &platform_error) ||
        images.images == NULL || images.count == 0) {
        stack_roots_fatal(scoop_platform_error_message(platform_error.code));
    }

    ScoopStackMapError parse_error;
    if (!scoop_stackmap_build_index(images.images, images.count, &stackmaps,
                                    &parse_error)) {
        fprintf(stderr,
                "scoop stackmap: image %zu offset %zu: %s\n",
                parse_error.image_index, parse_error.section_offset,
                scoop_stackmap_error_message(parse_error.code));
        abort();
    }
    for (size_t index = 0; index < stackmaps.record_count; index++) {
        platform_error = (ScoopPlatformError){0};
        if (!bundle->managed_frames->validate_record(&stackmaps.records[index],
                                                     &platform_error)) {
            fprintf(stderr,
                    "scoop stackmap: SafepointId %" PRIu64
                    " root %u: %s\n",
                    platform_error.safepoint_id,
                    (unsigned)platform_error.root_index,
                    scoop_platform_error_message(platform_error.code));
            abort();
        }
    }
    stackmaps_initialized = true;
}

const ScoopStackMapRecord *scoop_gc_stackmap_lookup(uintptr_t return_pc) {
    if (!stackmaps_initialized) {
        stack_roots_fatal("stack map lookup before initialization");
    }
    return scoop_stackmap_lookup(&stackmaps, return_pc);
}

static _Noreturn void managed_frame_fatal(ScoopPlatformError error) {
    fprintf(stderr,
            "scoop stackmap: SafepointId %" PRIu64 " root %u: %s\n",
            error.safepoint_id, (unsigned)error.root_index,
            scoop_platform_error_message(error.code));
    abort();
}

static void visit_managed_segment(const ScoopThreadState *thread,
                                  ScoopManagedAnchor cursor,
                                  uintptr_t boundary,
                                  ScoopGcRootVisitor visitor) {
    const ScoopPlatformBundle *bundle = scoop_platform_bundle();
    require_complete_bundle(bundle);
    if (thread == NULL || visitor.visit_slot == NULL ||
        cursor.return_pc == 0 || cursor.stack_pointer == 0 ||
        cursor.frame_pointer == 0 || cursor.previous != NULL ||
        boundary == 0) {
        stack_roots_fatal("managed segment has no exact anchor publication");
    }

    ScoopPlatformStackBounds bounds = {
        .low = thread->stack_low,
        .high = thread->stack_high,
    };

    for (;;) {
        const ScoopStackMapRecord *record =
            scoop_gc_stackmap_lookup(cursor.return_pc);
        if (record == NULL) {
            fprintf(stderr,
                    "scoop stackmap: no exact record for managed return PC "
                    "0x%" PRIxPTR "\n",
                    cursor.return_pc);
            abort();
        }

        ScoopManagedFrame frame;
        ScoopPlatformError error = {0};
        if (!bundle->managed_frames->frame_from_anchor(
                &cursor, record, bounds, &frame, &error)) {
            managed_frame_fatal(error);
        }
        for (uint16_t index = 0; index < record->root_count; index++) {
            void **slot = NULL;
            error = (ScoopPlatformError){0};
            if (!bundle->managed_frames->resolve_root(&frame, index, &slot,
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
        if (!bundle->managed_frames->next_frame(
                &frame, boundary, bounds, &return_pc, &stack_pointer,
                &frame_pointer, &has_next, &error)) {
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
    visit_managed_segment(thread, cursor,
                          (uintptr_t)thread->managed_stack_boundary,
                          visitor);
}

void scoop_gc_visit_managed_segment(const ScoopThreadState *thread,
                                    uintptr_t return_pc,
                                    uintptr_t stack_pointer,
                                    uintptr_t frame_pointer,
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
