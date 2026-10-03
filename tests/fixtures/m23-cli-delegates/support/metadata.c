#include <stdbool.h>

#include "scoop_runtime_metadata_v1.h"

extern const ScoopImageDescriptorV1 provider_image __asm__("PROVIDER_IMAGE");
extern const ScoopImageDescriptorV1 consumer_image __asm__("CONSUMER_IMAGE");

static bool matches_storage(const ScoopStaticStorageDescriptorV1 *storage) {
    if (M23_STORAGE_KIND == 1) {
        return storage->byte_size == 0 && storage->allocation_extent == 1 &&
               storage->scan_kind == SCOOP_STATIC_SCAN_NONE_V1;
    }
    if (M23_STORAGE_KIND == 2 || M23_STORAGE_KIND == 3) {
        const uint32_t scan =
            M23_STORAGE_KIND == 2 ? SCOOP_STATIC_SCAN_NONE_V1 : SCOOP_STATIC_SCAN_RECURSIVE_V1;
        return storage->byte_size == 24 && storage->scan_kind == scan;
    }
    return true;
}

int32_t m23_delegate_metadata(void) {
    for (uint64_t index = 0; index < provider_image.initialization_unit_count; index++) {
        const ScoopInitializationUnitDescriptorV1 *unit =
            provider_image.initialization_units[index];
        if (unit->registration.linkage_kind != SCOOP_REGISTRATION_LINKAGE_STRONG_V1) {
            return 0;
        }
    }
    if (consumer_image.initialization_unit_count != M23_EXPECTED_UNITS) {
        return 0;
    }
    for (uint64_t index = 0; index < consumer_image.initialization_unit_count; index++) {
        const ScoopInitializationUnitDescriptorV1 *unit =
            consumer_image.initialization_units[index];
        if (unit->registration.linkage_kind != SCOOP_REGISTRATION_LINKAGE_ODR_V1 ||
            unit->schedule_kind != SCOOP_INITIALIZATION_LAZY_ACCESS_V1 ||
            !matches_storage(unit->storage)) {
            return 0;
        }
    }
    return 1;
}
