#include <stdint.h>
#include <string.h>

#include "scoop_runtime_metadata_v1.h"

extern const ScoopImageDescriptorV1 left_image __asm__("LEFT_IMAGE");
extern const ScoopImageDescriptorV1 right_image __asm__("RIGHT_IMAGE");

int32_t m23_delegate_siblings(void) {
    if (left_image.initialization_unit_count != 2 || right_image.initialization_unit_count != 5) {
        return 0;
    }
    for (uint64_t left = 0; left < left_image.initialization_unit_count; left++) {
        const ScoopInitializationUnitDescriptorV1 *unit = left_image.initialization_units[left];
        if (unit->registration.linkage_kind != SCOOP_REGISTRATION_LINKAGE_ODR_V1) {
            return 0;
        }
        uint64_t matches = 0;
        for (uint64_t right = 0; right < right_image.initialization_unit_count; right++) {
            const ScoopInitializationUnitDescriptorV1 *other =
                right_image.initialization_units[right];
            if (memcmp(&unit->registration.semantic_id, &other->registration.semantic_id,
                       sizeof(unit->registration.semantic_id)) != 0) {
                continue;
            }
            matches++;
            if (unit != other || unit->cell != other->cell || unit->storage != other->storage ||
                unit->storage->writable_base != other->storage->writable_base ||
                unit->failure_root != other->failure_root ||
                unit->failure_root->writable_base != other->failure_root->writable_base ||
                other->registration.linkage_kind != SCOOP_REGISTRATION_LINKAGE_ODR_V1 ||
                memcmp(&unit->registration.odr_group_id, &other->registration.odr_group_id,
                       sizeof(unit->registration.odr_group_id)) != 0 ||
                memcmp(&unit->registration.odr_member_id, &other->registration.odr_member_id,
                       sizeof(unit->registration.odr_member_id)) != 0) {
                return 0;
            }
        }
        if (matches != 1) {
            return 0;
        }
    }
    return 1;
}
