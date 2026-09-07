#include <limits.h>
#include <stdio.h>
#include <stdlib.h>

#include "../platform.h"

#if !defined(__APPLE__) || !defined(__aarch64__)
#error "the Darwin/AArch64 profile requires macOS on AArch64"
#endif

typedef void (*ScoopTargetFunctionPointer)(void);

_Static_assert(CHAR_BIT == 8, "Scoop requires 8-bit bytes");
_Static_assert(UINTPTR_MAX == UINT64_MAX, "Scoop uintptr_t value width");
_Static_assert(sizeof(uintptr_t) == 8, "Scoop uintptr_t size");
_Static_assert(_Alignof(uintptr_t) == 8, "Scoop uintptr_t alignment");
_Static_assert(sizeof(void *) == 8, "Scoop data pointer size");
_Static_assert(_Alignof(void *) == 8, "Scoop data pointer alignment");
_Static_assert(sizeof(ScoopTargetFunctionPointer) == 8,
               "Scoop function pointer size");
_Static_assert(_Alignof(ScoopTargetFunctionPointer) == 8,
               "Scoop function pointer alignment");

extern const ScoopMetadataImageOps scoop_macho_metadata_image_ops;
extern const ScoopThreadVmOps scoop_darwin_thread_vm_ops;
extern const ScoopManagedFrameOps scoop_darwin_aarch64_managed_frame_ops;

static const ScoopPlatformBundle darwin_aarch64_bundle = {
    .metadata_images = &scoop_macho_metadata_image_ops,
    .thread_vm = &scoop_darwin_thread_vm_ops,
    .managed_frames = &scoop_darwin_aarch64_managed_frame_ops,
};

const ScoopPlatformBundle *scoop_platform_bundle(void) {
    return &darwin_aarch64_bundle;
}

const char *scoop_platform_error_message(ScoopPlatformErrorCode code) {
    switch (code) {
    case SCOOP_PLATFORM_OK:
        return "no error";
    case SCOOP_PLATFORM_METADATA_UNAVAILABLE:
        return "loaded image metadata is unavailable";
    case SCOOP_PLATFORM_INVALID_STACK_BOUNDS:
        return "OS thread stack bounds are invalid";
    case SCOOP_PLATFORM_INVALID_ANCHOR:
        return "managed entry anchor is invalid";
    case SCOOP_PLATFORM_INVALID_FRAME:
        return "managed frame chain is invalid";
    case SCOOP_PLATFORM_UNSUPPORTED_ROOT_LOCATION:
        return "stack map root location violates the target profile";
    case SCOOP_PLATFORM_ROOT_OUTSIDE_FRAME:
        return "stack map root lies outside its managed frame";
    case SCOOP_PLATFORM_VM_OPERATION_FAILED:
        return "virtual-memory operation failed";
    }
    return "unknown platform error";
}

ScoopPlatformStackBounds scoop_platform_stack_bounds(void) {
    ScoopPlatformStackBounds bounds;
    ScoopPlatformError error = {0};
    if (!darwin_aarch64_bundle.thread_vm->stack_bounds(&bounds, &error)) {
        fprintf(stderr, "scoop runtime: %s\n",
                scoop_platform_error_message(error.code));
        abort();
    }
    return bounds;
}
