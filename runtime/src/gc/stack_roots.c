#include <inttypes.h>
#include <stdio.h>
#include <stdlib.h>

#include "../platform/platform.h"
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
