#include <limits.h>

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
