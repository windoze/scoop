#include <mach-o/getsect.h>
#include <mach-o/loader.h>
#include <stddef.h>
#include <stdint.h>

#include "../platform.h"

#if !defined(__APPLE__) || !defined(__LP64__)
#error "the Mach-O image component requires 64-bit Darwin"
#endif

extern const struct mach_header_64 _mh_execute_header;

static bool macho_loaded_images(ScoopPlatformMetadataImages *images,
                                ScoopPlatformError *error) {
    static ScoopStackMapImage main_image;
    if (images == NULL || error == NULL) {
        return false;
    }
    unsigned long stackmap_size = 0;
    unsigned long text_size = 0;
    uint8_t *stackmaps = getsectiondata(
        &_mh_execute_header, "__LLVM_STACKMAPS", "__llvm_stackmaps",
        &stackmap_size);
    uint8_t *text =
        getsectiondata(&_mh_execute_header, "__TEXT", "__text", &text_size);
    if (stackmaps == NULL || stackmap_size == 0 || text == NULL ||
        text_size == 0 || text_size > UINTPTR_MAX - (uintptr_t)text) {
        error->code = SCOOP_PLATFORM_METADATA_UNAVAILABLE;
        return false;
    }
    main_image = (ScoopStackMapImage){
        .section = stackmaps,
        .section_size = stackmap_size,
        .text_start = (uintptr_t)text,
        .text_end = (uintptr_t)text + text_size,
    };
    *images = (ScoopPlatformMetadataImages){
        .images = &main_image,
        .count = 1,
    };
    return true;
}

const ScoopMetadataImageOps scoop_macho_metadata_image_ops = {
    .loaded_images = macho_loaded_images,
};
