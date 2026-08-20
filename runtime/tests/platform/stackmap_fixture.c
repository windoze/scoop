#include <assert.h>

#include "image_fixture.h"

ScoopStackMapIndex scoop_test_stackmaps(void) {
    ScoopPlatformMetadataImages images;
    ScoopPlatformError error = {0};
    ScoopStackMapError parse_error;
    ScoopStackMapIndex index;
    const ScoopPlatformBundle *platform = scoop_platform_bundle();
    assert(platform->metadata_images->loaded_images(&images, &error));
    assert(
        scoop_stackmap_build_index(images.images, images.count, &index, &parse_error));
    for (size_t record = 0; record < index.record_count; record++) {
        assert(
            platform->managed_frames->validate_record(&index.records[record], &error));
    }
    return index;
}
