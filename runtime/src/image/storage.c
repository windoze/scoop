#include <stdlib.h>

#include "../value_shape.h"
#include "internal.h"

static void check_storage(ScoopImageRegistry *registry, size_t index) {
    ScoopMetadataCheck check =
        scoop_record_check(registry, SCOOP_RECORD_STORAGE, index);
    const ScoopStaticStorageDescriptorV1 *storage =
        registry->tables[SCOOP_RECORD_STORAGE].entries[index].record;
    uint64_t extent = storage->byte_size == 0 ? 1 : storage->byte_size;
    if (storage->allocation_extent != extent ||
        storage->required_alignment > SCOOP_MAXIMUM_MANAGED_ALIGNMENT ||
        scoop_digest_zero(&storage->scan_fingerprint) ||
        scoop_digest_zero(&storage->layout_fingerprint)) {
        scoop_metadata_fatal(&check,
                             "allocation extent, alignment or fingerprint");
    }
    scoop_metadata_writable(&check, storage->writable_base, extent,
                            storage->required_alignment,
                            "writable allocation range");
    scoop_metadata_readonly(&check, storage->scan_program, 1, 8, 8,
                            "scan header range");
    scoop_metadata_scan(registry, &check, storage->scan_program);
    if (storage->scan_kind == SCOOP_STATIC_SCAN_NONE_V1) {
        if (storage->scan_program[0] != 0) {
            scoop_metadata_fatal(&check, "None scan has references");
        }
    } else if (storage->scan_kind == SCOOP_STATIC_SCAN_RECURSIVE_V1) {
        scoop_shape_scan_validate(storage->scan_program, storage->byte_size);
        registry->static_roots[registry->static_root_count++] = storage;
    } else {
        scoop_metadata_fatal(&check, "unknown scan kind");
    }
    scoop_metadata_bytes(&check, storage->initial_template,
                         "initial template span");
    scoop_metadata_readonly(
        &check, storage->initial_relocations, storage->initial_relocation_count,
        sizeof *storage->initial_relocations, 8, "initial relocation span");
    if (storage->byte_size == 0 &&
        (storage->scan_kind != SCOOP_STATIC_SCAN_NONE_V1 ||
         *(const uint8_t *)storage->writable_base != 0)) {
        scoop_metadata_fatal(&check, "ZST storage token or scan");
    }
    switch (storage->initial_state_kind) {
    case SCOOP_STATIC_INITIAL_ZEROED_FOR_RUNTIME_UNIT_V1:
        if (storage->initial_template.length != 0 ||
            storage->initial_relocation_count != 0) {
            scoop_metadata_fatal(&check,
                                 "zeroed storage has encoded initial values");
        }
        for (uint64_t byte = 0; byte < extent; byte++) {
            if (((const uint8_t *)storage->writable_base)[byte] != 0) {
                scoop_metadata_fatal(&check, "nonzero initial storage");
            }
        }
        break;
    case SCOOP_STATIC_INITIAL_ENCODED_VALUE_V1:
        if (storage->initial_template.length != storage->byte_size) {
            scoop_metadata_fatal(&check, "encoded template size");
        }
        scoop_image_initial_values(registry, index);
        break;
    default:
        scoop_metadata_fatal(&check, "unknown initial state kind");
    }
}

void scoop_image_validate_storage_and_units(ScoopImageRegistry *registry) {
    if (registry->type_addresses == NULL || registry->static_roots != NULL) {
        scoop_metadata_fatal(NULL, "storage registration order");
    }
    scoop_image_immortals(registry);
    registry->static_roots =
        scoop_metadata_allocate(registry->tables[SCOOP_RECORD_STORAGE].count,
                                sizeof *registry->static_roots);
    for (size_t index = 0; index < registry->tables[SCOOP_RECORD_STORAGE].count;
         index++) {
        check_storage(registry, index);
    }
    scoop_image_units(registry);
    scoop_image_allocation_ranges(registry);
}
