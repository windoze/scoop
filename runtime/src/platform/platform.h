#ifndef SCOOP_PLATFORM_H
#define SCOOP_PLATFORM_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#include "../gc/stackmap.h"

/* Platform-neutral runtime capabilities. Generic runtime modules include
 * only this header; target-specific headers stay below platform/. */

typedef struct ScoopPlatformStackBounds {
    const char *low;
    const char *high;
} ScoopPlatformStackBounds;

typedef struct ScoopPlatformMetadataImages {
    const ScoopStackMapImage *images;
    size_t count;
} ScoopPlatformMetadataImages;

typedef struct ScoopManagedAnchor {
    uintptr_t return_pc;
    uintptr_t stack_pointer;
    uintptr_t frame_pointer;
    struct ScoopManagedAnchor *previous;
} ScoopManagedAnchor;

typedef struct ScoopManagedFrame {
    uintptr_t return_pc;
    uintptr_t stack_pointer;
    uintptr_t frame_pointer;
    const ScoopStackMapRecord *record;
} ScoopManagedFrame;

typedef enum ScoopPlatformErrorCode {
    SCOOP_PLATFORM_OK = 0,
    SCOOP_PLATFORM_METADATA_UNAVAILABLE,
    SCOOP_PLATFORM_INVALID_STACK_BOUNDS,
    SCOOP_PLATFORM_INVALID_ANCHOR,
    SCOOP_PLATFORM_INVALID_FRAME,
    SCOOP_PLATFORM_UNSUPPORTED_ROOT_LOCATION,
    SCOOP_PLATFORM_ROOT_OUTSIDE_FRAME,
    SCOOP_PLATFORM_VM_OPERATION_FAILED,
} ScoopPlatformErrorCode;

typedef struct ScoopPlatformError {
    ScoopPlatformErrorCode code;
    uint64_t safepoint_id;
    uint16_t root_index;
} ScoopPlatformError;

typedef struct ScoopMetadataImageOps {
    bool (*loaded_images)(ScoopPlatformMetadataImages *images,
                          ScoopPlatformError *error);
} ScoopMetadataImageOps;

typedef struct ScoopThreadVmOps {
    bool (*stack_bounds)(ScoopPlatformStackBounds *bounds,
                         ScoopPlatformError *error);
    size_t (*page_size)(void);
    bool (*protect_none)(void *base, size_t size,
                         ScoopPlatformError *error);
} ScoopThreadVmOps;

typedef struct ScoopManagedFrameOps {
    bool (*validate_record)(const ScoopStackMapRecord *record,
                            ScoopPlatformError *error);
    bool (*frame_from_anchor)(const ScoopManagedAnchor *anchor,
                              const ScoopStackMapRecord *record,
                              ScoopPlatformStackBounds bounds,
                              ScoopManagedFrame *frame,
                              ScoopPlatformError *error);
    bool (*resolve_root)(const ScoopManagedFrame *frame, uint16_t root_index,
                         void ***slot, ScoopPlatformError *error);
    bool (*next_frame)(const ScoopManagedFrame *frame,
                       uintptr_t managed_boundary,
                       ScoopPlatformStackBounds bounds,
                       uintptr_t *return_pc, uintptr_t *stack_pointer,
                       uintptr_t *frame_pointer, bool *has_next,
                       ScoopPlatformError *error);
} ScoopManagedFrameOps;

typedef struct ScoopPlatformBundle {
    const ScoopMetadataImageOps *metadata_images;
    const ScoopThreadVmOps *thread_vm;
    const ScoopManagedFrameOps *managed_frames;
} ScoopPlatformBundle;

/* The selected target profile supplies exactly one complete immutable
 * bundle. Generic runtime code must not select components with host macros. */
const ScoopPlatformBundle *scoop_platform_bundle(void);
const char *scoop_platform_error_message(ScoopPlatformErrorCode code);

/* Convenience wrapper retained for thread registration. It still dispatches
 * through the selected bundle and treats an incomplete platform as fatal. */
ScoopPlatformStackBounds scoop_platform_stack_bounds(void);

#endif /* SCOOP_PLATFORM_H */
