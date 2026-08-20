#include <assert.h>
#include <stdlib.h>
#include <string.h>

#include "../../src/gc/gc_internal.h"
#include "../../src/image/internal.h"
#include "../../src/value_shape.h"
#include "image_fixture.h"

void scoop_test_image_init(const ScoopTypeDescriptor *const *types, size_t type_count,
                           const ScoopStaticStorageDescriptorV1 *const *roots,
                           size_t root_count,
                           const ScoopImmortalObjectDescriptorV1 *const *immortals,
                           size_t immortal_count) {
    static ScoopImageRegistry registry;
    ScoopRecordTable *type_table = &registry.tables[SCOOP_RECORD_TYPE];
    ScoopTypeRegistrationDescriptorV1 *registrations =
        calloc(type_count + 1, sizeof *registrations);
    type_table->entries = calloc(type_count + 1, sizeof *type_table->entries);
    registry.type_addresses = calloc(type_count + 1, sizeof *registry.type_addresses);
    assert(registrations && type_table->entries && registry.type_addresses);
    type_table->count = type_count;
    for (size_t index = 0; index < type_count; index++) {
        scoop_shape_validate(types[index]);
        registrations[index].descriptor = types[index];
        type_table->entries[index].record = &registrations[index];
        registry.type_addresses[index] =
            (ScoopRecordAddress){(uintptr_t)types[index], index};
    }
    qsort(registry.type_addresses, type_count, sizeof *registry.type_addresses,
          scoop_record_address_compare);
    registry.static_roots = calloc(root_count + 1, sizeof *registry.static_roots);
    assert(registry.static_roots);
    registry.static_root_count = root_count;
    for (size_t index = 0; index < root_count; index++) {
        registry.static_roots[index] = roots[index];
    }
    ScoopRecordTable *immortal_table = &registry.tables[SCOOP_RECORD_IMMORTAL];
    immortal_table->entries =
        calloc(immortal_count + 1, sizeof *immortal_table->entries);
    registry.immortal_addresses =
        calloc(immortal_count + 1, sizeof *registry.immortal_addresses);
    assert(immortal_table->entries && registry.immortal_addresses);
    immortal_table->count = immortal_count;
    for (size_t index = 0; index < immortal_count; index++) {
        immortal_table->entries[index].record = immortals[index];
        registry.immortal_addresses[index] =
            (ScoopRecordAddress){(uintptr_t)immortals[index]->object_start, index};
    }
    qsort(registry.immortal_addresses, immortal_count,
          sizeof *registry.immortal_addresses, scoop_record_address_compare);
    registry.stackmaps = scoop_test_stackmaps();
    scoop_image_publish(&registry);
    scoop_gc_stackmaps_init(&registry.stackmaps);
    scoop_gc_heap_init();
}
