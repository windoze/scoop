#include <assert.h>
#include <string.h>

#include "image_stackmap_fixture.h"

void scoop_test_stackmap_write(ScoopStackmapFixture *fixture, size_t offset,
                               uint64_t value, size_t size) {
    assert(offset + size <= sizeof fixture->storage.data->stackmaps);
    for (size_t byte = 0; byte < size; byte++) {
        fixture->storage.data->stackmaps[offset + byte] =
            (uint8_t)(value >> (byte * 8));
    }
}

static void append(ScoopStackmapFixture *fixture, uint64_t value, size_t size) {
    scoop_test_stackmap_write(fixture, fixture->length, value, size);
    fixture->length += size;
}

static void align_blob(ScoopStackmapFixture *fixture) {
    while (fixture->length % 8)
        append(fixture, 0, 1);
}

static void location(ScoopStackmapFixture *fixture, uint8_t kind, uint16_t reg,
                     int32_t offset) {
    append(fixture, kind, 1);
    append(fixture, 0, 1);
    append(fixture, 8, 2);
    append(fixture, reg, 2);
    append(fixture, 0, 2);
    append(fixture, (uint32_t)offset, 4);
}

void scoop_test_stackmap_fingerprint(ScoopSafepointRegistrationDescriptorV1 *site,
                                     uint32_t instruction_offset) {
    ScoopStackMapRootPair pair = {.base = {.kind = SCOOP_STACKMAP_INDIRECT,
                                           .size = 8,
                                           .dwarf_register = 31,
                                           .offset = 16},
                                  .derived = {.kind = SCOOP_STACKMAP_INDIRECT,
                                              .size = 8,
                                              .dwarf_register = 31,
                                              .offset = 16}};
    ScoopStackMapLiveOut live_out = {19, 8};
    ScoopStackMapRecord record = {.safepoint_id = site->safepoint_id,
                                  .instruction_offset = instruction_offset,
                                  .stack_size = 64,
                                  .root_count = (uint16_t)site->root_pair_count,
                                  .roots = &pair,
                                  .live_out_count = (uint16_t)site->root_pair_count,
                                  .live_outs = &live_out};
    for (size_t index = 0; index < 3; index++) {
        record.header[index] =
            (ScoopStackMapLocation){.kind = SCOOP_STACKMAP_CONSTANT, .size = 8};
    }
    assert(scoop_stackmap_fingerprint(&record, site,
                                      &site->normalized_stackmap_fingerprint));
}

static BlobOffsets blob(ScoopStackmapFixture *fixture, size_t site_index,
                        bool indexed) {
    StorageMetadata *data = fixture->storage.data;
    const ScoopSafepointRegistrationDescriptorV1 *site = &data->sites[site_index];
    uint32_t instruction_offset = site_index == 0 ? 8 : 4;
    ScoopCallableAddressV1 entry = data->callables[site_index == 0 ? 1 : 0].entry;
    BlobOffsets result = {.start = fixture->length};
    append(fixture, 3, 4);
    append(fixture, 1, 4);
    append(fixture, indexed ? 1 : 0, 4);
    append(fixture, 1, 4);
    append(fixture, (uintptr_t)entry, 8);
    append(fixture, 64, 8);
    append(fixture, 1, 8);
    if (indexed)
        append(fixture, 0, 8);
    result.record = fixture->length;
    append(fixture, site->safepoint_id, 8);
    append(fixture, instruction_offset, 4);
    append(fixture, 0, 2);
    append(fixture, 3 + 2 * site->root_pair_count, 2);
    result.locations = fixture->length;
    location(fixture, indexed ? SCOOP_STACKMAP_CONSTANT_INDEX : SCOOP_STACKMAP_CONSTANT,
             0, 0);
    location(fixture, SCOOP_STACKMAP_CONSTANT, 0, 0);
    location(fixture, SCOOP_STACKMAP_CONSTANT, 0, 0);
    for (size_t index = 0; index < site->root_pair_count; index++) {
        location(fixture, SCOOP_STACKMAP_INDIRECT, 31, 16);
        location(fixture, SCOOP_STACKMAP_INDIRECT, 31, 16);
    }
    align_blob(fixture);
    append(fixture, 0, 2);
    append(fixture, site->root_pair_count, 2);
    result.live_outs = fixture->length;
    for (size_t index = 0; index < site->root_pair_count; index++) {
        append(fixture, 19, 2);
        append(fixture, 0, 1);
        append(fixture, 8, 1);
    }
    align_blob(fixture);
    result.end = fixture->length;
    return result;
}

void scoop_test_stackmap_setup(ScoopStackmapFixture *fixture) {
    scoop_test_storage_setup(&fixture->storage, false);
    StorageMetadata *data = fixture->storage.data;
    for (size_t index = 0; index < 2; index++) {
        data->sites[index] = (ScoopSafepointRegistrationDescriptorV1){
            .prefix = {SCOOP_SAFEPOINT_REGISTRATION_DESCRIPTOR_MAGIC_V1,
                       SCOOP_RUNTIME_METADATA_ABI_VERSION_V1,
                       sizeof data->sites[index]},
            .registration = {.linkage_kind = SCOOP_REGISTRATION_LINKAGE_STRONG_V1,
                             .semantic_id = {{(uint8_t)(77 + index)}}},
            .safepoint_id = 11 + index,
            .site_role = SCOOP_SAFEPOINT_MANAGED_POLL_V1,
            .root_pair_count = index == 0 ? 1 : 0,
            .owner_callable_id = {{(uint8_t)(index == 0 ? 2 : 1)}}};
        data->site_table[index] = &data->sites[index];
        scoop_test_stackmap_fingerprint(&data->sites[index], index == 0 ? 8 : 4);
    }
    data->sites[0].registration.linkage_kind = SCOOP_REGISTRATION_LINKAGE_ODR_V1;
    data->sites[0].registration.odr_group_id.bytes[0] = 55;
    data->sites[0].registration.odr_member_id.bytes[0] = 77;
    data->callables[1].registration.linkage_kind = SCOOP_REGISTRATION_LINKAGE_ODR_V1;
    data->callables[1].registration.odr_group_id.bytes[0] = 55;
    data->callables[1].registration.odr_member_id.bytes[0] = 2;
    data->image.safepoints = data->site_table;
    data->image.safepoint_count = 2;
    fixture->blobs[0] = blob(fixture, 0, false);
    fixture->blobs[1] = blob(fixture, 1, false);
    fixture->blobs[2] = blob(fixture, 0, true);
    uintptr_t low = UINTPTR_MAX, high = 0;
    for (size_t index = 0; index < 8; index++) {
        uintptr_t entry = (uintptr_t)data->callables[index].entry;
        if (entry < low)
            low = entry;
        if (entry > high)
            high = entry;
    }
    fixture->image =
        (ScoopStackMapImage){data->stackmaps, fixture->length, low, high + 4};
    fixture->storage.loaded.images = &fixture->image;
    fixture->storage.loaded.count = 1;
}
