#ifndef SCOOP_TEST_IMAGE_STORAGE_FIXTURE_H
#define SCOOP_TEST_IMAGE_STORAGE_FIXTURE_H

#include "../src/image/internal.h"
#include "../src/value_shape.h"

enum {
    ROOT_FAILURE,
    PROPERTY_A,
    FAILURE_A,
    ENCODED_REF,
    ZST_TOKEN,
    PROPERTY_B,
    FAILURE_B,
    EXTRA_STORAGE,
    STORAGE_COUNT
};

typedef struct StorageState {
    uint64_t slots[STORAGE_COUNT];
    ScoopInitializationCell cells[2];
    uint64_t aggregate[4];
} StorageState;

typedef struct StorageMetadata {
    ScoopImageDescriptorV1 image;
    const ScoopImageDescriptorV1 *input[1];
    ScoopRootEntryDescriptorV1 root;
    uint64_t empty[1];
    char text[8];
    _Alignas(8) uint8_t td_bytes[144];
    ScoopTypeRegistrationDescriptorV1 type;
    const ScoopTypeRegistrationDescriptorV1 *types[1];
    struct {
        ScoopObjectHeader header;
        uint64_t length;
        uint8_t bytes[8];
    } string_object;
    ScoopImmortalObjectDescriptorV1 immortals[2];
    const ScoopImmortalObjectDescriptorV1 *immortal_table[2];
    ScoopStaticStorageDescriptorV1 storages[STORAGE_COUNT];
    const ScoopStaticStorageDescriptorV1 *storage_table[STORAGE_COUNT];
    uint8_t templates[STORAGE_COUNT][32];
    ScoopStaticImmortalRelocationV1 relocations[4];
    uint64_t reference_scan[2];
    uint64_t scan_nodes[3][5];
    ScoopInitializationUnitDescriptorV1 units[2];
    const ScoopInitializationUnitDescriptorV1 *unit_table[2];
    ScoopCallableRegistrationDescriptorV1 callables[8];
    const ScoopCallableRegistrationDescriptorV1 *callable_table[8];
} StorageMetadata;

typedef struct ScoopStorageFixture {
    StorageMetadata *data;
    StorageState *state;
    size_t size, page;
    ScoopPlatformImageRange ranges[3];
    ScoopPlatformMetadataImages loaded;
} ScoopStorageFixture;

void scoop_test_storage_setup(ScoopStorageFixture *fixture, bool both_eager);
void scoop_test_storage_dispose(ScoopStorageFixture *fixture);

#endif
