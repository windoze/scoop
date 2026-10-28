#include <stdint.h>
#include <string.h>

#include "scoop_runtime_metadata_v1.h"

extern const ScoopImageDescriptorV1 left_image __asm__("LEFT_IMAGE");
extern const ScoopImageDescriptorV1 right_image __asm__("RIGHT_IMAGE");
extern const ScoopImageDescriptorV1 consumer_image __asm__("CONSUMER_IMAGE");

int32_t m23_delegate_siblings(void) {
    const ScoopImageDescriptorV1 *images[] = {&left_image, &right_image, &consumer_image};
    if (left_image.initialization_unit_count + right_image.initialization_unit_count +
            consumer_image.initialization_unit_count !=
        5) {
        return 0;
    }
    const ScoopInitializationUnitDescriptorV1 *selected[5];
    uint64_t count = 0;
    /* ODR selection may retain a unit in any consumer of the generic declaration. */
    for (uint64_t image_index = 0; image_index < 3; image_index++) {
        const ScoopImageDescriptorV1 *image = images[image_index];
        for (uint64_t index = 0; index < image->initialization_unit_count; index++) {
            const ScoopInitializationUnitDescriptorV1 *unit = image->initialization_units[index];
            if (unit->registration.linkage_kind != SCOOP_REGISTRATION_LINKAGE_ODR_V1) {
                return 0;
            }
            for (uint64_t previous = 0; previous < count; previous++) {
                const ScoopInitializationUnitDescriptorV1 *other = selected[previous];
                if (memcmp(&unit->registration.semantic_id, &other->registration.semantic_id,
                           sizeof(unit->registration.semantic_id)) == 0 ||
                    unit->cell == other->cell || unit->storage == other->storage ||
                    unit->storage->writable_base == other->storage->writable_base ||
                    unit->failure_root == other->failure_root ||
                    unit->failure_root->writable_base == other->failure_root->writable_base) {
                    return 0;
                }
            }
            selected[count++] = unit;
        }
    }
    return 1;
}
