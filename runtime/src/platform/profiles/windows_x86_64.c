#include <limits.h>

#include "../platform.h"

#if !defined(_WIN64) || !defined(__x86_64__)
#error "the Windows/amd64 profile requires 64-bit Windows on x86_64"
#endif

typedef void (*ScoopTargetFunctionPointer)(void);

_Static_assert(CHAR_BIT == 8, "Scoop requires 8-bit bytes");
_Static_assert(sizeof(int) == 4 && sizeof(long) == 4, "Windows uses LLP64");
_Static_assert(UINTPTR_MAX == UINT64_MAX, "Scoop uintptr_t value width");
_Static_assert(sizeof(uintptr_t) == 8, "Scoop uintptr_t size");
_Static_assert(_Alignof(uintptr_t) == 8, "Scoop uintptr_t alignment");
_Static_assert(sizeof(void *) == 8, "Scoop data pointer size");
_Static_assert(_Alignof(void *) == 8, "Scoop data pointer alignment");
_Static_assert(sizeof(ScoopTargetFunctionPointer) == 8, "Scoop function pointer size");
_Static_assert(_Alignof(ScoopTargetFunctionPointer) == 8, "Scoop function pointer alignment");

extern const ScoopMetadataImageOps scoop_pe_metadata_image_ops;
extern const ScoopThreadVmOps scoop_windows_thread_vm_ops;
extern const ScoopManagedFrameOps scoop_windows_amd64_managed_frame_ops;

static const ScoopPlatformBundle windows_amd64_bundle = {
    .metadata_images = &scoop_pe_metadata_image_ops,
    .thread_vm = &scoop_windows_thread_vm_ops,
    .managed_frames = &scoop_windows_amd64_managed_frame_ops,
};

const ScoopPlatformBundle *scoop_platform_bundle(void) { return &windows_amd64_bundle; }
