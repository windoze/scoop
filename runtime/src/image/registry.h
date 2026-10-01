#ifndef SCOOP_IMAGE_REGISTRY_H
#define SCOOP_IMAGE_REGISTRY_H

#include "../platform/platform.h"
#include "scoop_runtime_metadata_v1.h"

/* Each table has its own semantic ID namespace. These are the six concrete
 * metadata kinds in the runtime ABI, not extensible registration providers. */
typedef enum ScoopRecordKind {
    SCOOP_RECORD_STORAGE,
    SCOOP_RECORD_IMMORTAL,
    SCOOP_RECORD_UNIT,
    SCOOP_RECORD_TYPE,
    SCOOP_RECORD_SITE,
    SCOOP_RECORD_CALLABLE,
    SCOOP_RECORD_KIND_COUNT
} ScoopRecordKind;

typedef struct ScoopRegisteredRecord {
    const void *record;
    const ScoopRegistrationIdentityV1 *identity;
    const ScoopImageDescriptorV1 *producer;
} ScoopRegisteredRecord;

typedef struct ScoopRecordAddress {
    uintptr_t address;
    size_t index;
} ScoopRecordAddress;

typedef struct ScoopRecordTable {
    ScoopRegisteredRecord *entries;
    ScoopRecordAddress *addresses;
    size_t count;
} ScoopRecordTable;

typedef struct ScoopImageRegistry {
    const ScoopPlatformMetadataImages *loaded;
    const ScoopImageDescriptorV1 **images;
    const ScoopImageDescriptorV1 **image_order;
    size_t image_count;
    const ScoopRootEntryDescriptorV1 *root;
    ScoopRecordTable tables[SCOOP_RECORD_KIND_COUNT];
    ScoopRecordAddress *type_addresses;
    ScoopRecordAddress *callable_addresses;
    ScoopRecordAddress *site_ids;
    ScoopRecordAddress *immortal_addresses;
    const ScoopStaticStorageDescriptorV1 **static_roots;
    size_t static_root_count;
    const ScoopInitializationUnitDescriptorV1 **eager_units;
    size_t eager_unit_count;
    struct ScoopScanRanges *scan_ranges;
    ScoopStackMapIndex stackmaps;
} ScoopImageRegistry;

/* Collection has no managed side effects. Cross-record GC and initialization
 * relations are resolved on the complete collection before publication. */
ScoopImageRegistry *scoop_image_collect(const ScoopPlatformMetadataImages *loaded,
                                        const ScoopImageDescriptorV1 *const *images,
                                        uint64_t image_count,
                                        const ScoopRootEntryDescriptorV1 *root);
void scoop_image_registry_dispose(ScoopImageRegistry *registry);
void scoop_image_validate_code_and_types(ScoopImageRegistry *registry);
const ScoopTypeRegistrationDescriptorV1 *
scoop_image_type(const ScoopImageRegistry *registry, const ScoopTypeDescriptor *td);
void scoop_image_validate_storage_and_units(ScoopImageRegistry *registry);
const ScoopImmortalObjectDescriptorV1 *
scoop_image_immortal(const ScoopImageRegistry *registry, const void *object);
void scoop_image_stackmaps(ScoopImageRegistry *registry,
                           const ScoopManagedFrameOps *frames);

#endif
