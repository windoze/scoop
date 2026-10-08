#include <assert.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <unistd.h>

#include "image_storage_fixture.h"

static void source(void) {}
static void initialize_a(void) {}
static void ensure_a(void) {}
static uint32_t gateway_a(void) { return 0; }
static void initialize_b(void) {}
static void ensure_b(void) {}
static uint32_t root_gateway(int32_t argc, const char *const *argv, int32_t *exit_code) {
    (void)argc;
    (void)argv;
    *exit_code = 0;
    return 0;
}
static uint32_t gateway_b(void) { return 0; }

static ScoopDescriptorPrefixV1 prefix(uint64_t magic, size_t size) {
    return (ScoopDescriptorPrefixV1){
        magic, SCOOP_RUNTIME_METADATA_ABI_VERSION_V1, (uint32_t)size};
}

static ScoopRegistrationIdentityV1 identity(uint8_t id) {
    return (ScoopRegistrationIdentityV1){
        .linkage_kind = SCOOP_REGISTRATION_LINKAGE_STRONG_V1,
        .semantic_id = {{id}}};
}

static int range_order(const void *left, const void *right) {
    uintptr_t a = ((const ScoopPlatformImageRange *)left)->start;
    uintptr_t b = ((const ScoopPlatformImageRange *)right)->start;
    return (a > b) - (a < b);
}

static void setup_records(ScoopStorageFixture *fixture, bool both_eager) {
    StorageMetadata *data = fixture->data;
    ScoopByteSpanV1 name = {(const uint8_t *)data->text, 4};
    ScoopTypeDescriptor *td = (ScoopTypeDescriptor *)data->td_bytes;
    *td = (ScoopTypeDescriptor){
        .type_id = 1,
        .diagnostic_name = name,
        .instance_shape = {.instance_kind = SCOOP_TYPE_INSTANCE_INLINE_BYTES_V1,
                           .inline_storage_kind =
                               SCOOP_INLINE_STORAGE_INLINE_V1,
                           .minimum_size = 24,
                           .instance_alignment = 8,
                           .inline_offset = 24,
                           .inline_size = 1,
                           .inline_stride = 1,
                           .inline_alignment = 1}};
    data->type = (ScoopTypeRegistrationDescriptorV1){
        .prefix = prefix(SCOOP_TYPE_REGISTRATION_DESCRIPTOR_MAGIC_V1,
                         sizeof data->type),
        .registration = identity(1),
        .runtime_type_id = 1,
        .descriptor = td,
        .descriptor_fingerprint = {{1}},
        .layout_fingerprint = {{1}}};
    data->types[0] = &data->type;
    data->string_object.header.td = td;
    data->string_object.length = 3;
    memcpy(data->string_object.bytes, "abc", 3);
    data->immortals[0] = (ScoopImmortalObjectDescriptorV1){
        .prefix = prefix(SCOOP_IMMORTAL_OBJECT_DESCRIPTOR_MAGIC_V1,
                         sizeof data->immortals[0]),
        .registration = identity(1),
        .object_start = &data->string_object,
        .object_size = sizeof data->string_object,
        .required_alignment = 8,
        .type_registration = &data->type};
    data->immortal_table[0] = &data->immortals[0];
    data->reference_scan[0] = 1;
    for (size_t index = 0; index < STORAGE_COUNT; index++) {
        bool reference = index == ROOT_FAILURE || index == FAILURE_A ||
                         index == FAILURE_B || index == ENCODED_REF;
        data->storages[index] = (ScoopStaticStorageDescriptorV1){
            .prefix = prefix(SCOOP_STATIC_STORAGE_DESCRIPTOR_MAGIC_V1,
                             sizeof data->storages[index]),
            .registration = identity((uint8_t)(index + 1)),
            .scan_kind = reference ? SCOOP_STATIC_SCAN_RECURSIVE_V1
                                   : SCOOP_STATIC_SCAN_NONE_V1,
            .initial_state_kind =
                SCOOP_STATIC_INITIAL_ZEROED_FOR_RUNTIME_UNIT_V1,
            .writable_base = &fixture->state->slots[index],
            .byte_size = 8,
            .allocation_extent = 8,
            .required_alignment = 8,
            .scan_program = reference ? data->reference_scan : data->empty,
            .scan_fingerprint = {{1}},
            .layout_fingerprint = {{1}},
            .initial_template = {(const uint8_t *)data->empty, 0},
            .initial_relocations = (const void *)data->empty};
        data->storage_table[index] = &data->storages[index];
    }
    data->storages[ENCODED_REF].initial_state_kind =
        SCOOP_STATIC_INITIAL_ENCODED_VALUE_V1;
    data->storages[ENCODED_REF].initial_template =
        (ScoopByteSpanV1){data->templates[ENCODED_REF], 8};
    data->storages[ENCODED_REF].initial_relocations = data->relocations;
    data->storages[ENCODED_REF].initial_relocation_count = 1;
    data->relocations[0] =
        (ScoopStaticImmortalRelocationV1){0, &data->immortals[0]};
    fixture->state->slots[ENCODED_REF] = (uintptr_t)&data->string_object;
    data->storages[ZST_TOKEN].byte_size = 0;
    data->storages[ZST_TOKEN].allocation_extent = 1;
    data->storages[ZST_TOKEN].required_alignment = 1;
    data->storages[ZST_TOKEN].initial_state_kind =
        SCOOP_STATIC_INITIAL_ENCODED_VALUE_V1;
    ScoopCallableAddressV1 bodies[] = {source,
                                       initialize_a,
                                       ensure_a,
                                       (ScoopCallableAddressV1)gateway_a,
                                       initialize_b,
                                       ensure_b,
                                       (ScoopCallableAddressV1)root_gateway,
                                       (ScoopCallableAddressV1)gateway_b};
    uintptr_t low = (uintptr_t)bodies[0], high = low;
    for (size_t index = 0; index < 8; index++) {
        data->callables[index] = (ScoopCallableRegistrationDescriptorV1){
            .prefix = prefix(SCOOP_CALLABLE_REGISTRATION_DESCRIPTOR_MAGIC_V1,
                             sizeof data->callables[index]),
            .registration = identity((uint8_t)(index + 1)),
            .body_definition_fingerprint = {{(uint8_t)(index + 1)}},
            .entry = bodies[index]};
        data->callable_table[index] = &data->callables[index];
        uintptr_t address = (uintptr_t)bodies[index];
        if (address < low)
            low = address;
        if (address > high)
            high = address;
    }
    fixture->ranges[2] = (ScoopPlatformImageRange){
        low, high + 4, SCOOP_IMAGE_READ | SCOOP_IMAGE_EXECUTE};
    for (size_t index = 0; index < 2; index++) {
        bool eager = index == 0 || both_eager;
        data->units[index] = (ScoopInitializationUnitDescriptorV1){
            .prefix = prefix(SCOOP_INITIALIZATION_UNIT_DESCRIPTOR_MAGIC_V1,
                             sizeof data->units[index]),
            .registration = identity((uint8_t)(2 - index)),
            .schedule_kind = eager ? SCOOP_INITIALIZATION_EAGER_STARTUP_V1
                                   : SCOOP_INITIALIZATION_LAZY_ACCESS_V1,
            .diagnostic_path = name,
            .cell = &fixture->state->cells[index],
            .storage = &data->storages[index == 0 ? PROPERTY_A : PROPERTY_B],
            .failure_root = &data->storages[index == 0 ? FAILURE_A : FAILURE_B],
            .initializer_callable_id = {{(uint8_t)(index == 0 ? 2 : 5)}},
            .ensure_callable_id = {{(uint8_t)(index == 0 ? 3 : 6)}},
            .initializer_entry = index == 0 ? initialize_a : initialize_b,
            .ensure_entry = index == 0 ? ensure_a : ensure_b};
        if (eager) {
            data->units[index].startup_gateway_callable_id.bytes[0] =
                index == 0 ? 4 : 8;
            data->units[index].startup_gateway_definition_fingerprint.bytes[0] =
                index == 0 ? 4 : 8;
            data->units[index].startup_gateway =
                index == 0 ? gateway_a : gateway_b;
        }
        data->unit_table[index] = &data->units[index];
    }
}

void scoop_test_storage_setup(ScoopStorageFixture *fixture, bool both_eager) {
    fixture->page = (size_t)sysconf(_SC_PAGESIZE);
    fixture->size = (sizeof(StorageMetadata) + fixture->page - 1) /
                    fixture->page * fixture->page;
    fixture->data = mmap(NULL, fixture->size + fixture->page,
                         PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANON, -1, 0);
    fixture->state = mmap(NULL, fixture->page, PROT_READ | PROT_WRITE,
                          MAP_PRIVATE | MAP_ANON, -1, 0);
    assert(fixture->data != MAP_FAILED && fixture->state != MAP_FAILED);
    assert(mprotect((uint8_t *)fixture->data + fixture->size, fixture->page,
                    PROT_NONE) == 0);
    fixture->ranges[0] = (ScoopPlatformImageRange){
        (uintptr_t)fixture->data, (uintptr_t)fixture->data + fixture->size,
        SCOOP_IMAGE_READ};
    fixture->ranges[1] = (ScoopPlatformImageRange){
        (uintptr_t)fixture->state, (uintptr_t)fixture->state + fixture->page,
        SCOOP_IMAGE_READ | SCOOP_IMAGE_WRITE};
    StorageMetadata *data = fixture->data;
    strcpy(data->text, "test");
    setup_records(fixture, both_eager);
    qsort(fixture->ranges, 3, sizeof *fixture->ranges, range_order);
    fixture->loaded = (ScoopPlatformMetadataImages){.ranges = fixture->ranges,
                                                    .range_count = 3};
    ScoopByteSpanV1 name = {(const uint8_t *)data->text, 4};
    data->image = (ScoopImageDescriptorV1){
        .prefix = prefix(SCOOP_IMAGE_DESCRIPTOR_MAGIC_V1, sizeof data->image),
        .cone = {name, name, name, {{1}}},
        .runtime_image_fingerprint = {{1}},
        .dependencies = (const void *)data->empty,
        .static_storages = data->storage_table,
        .static_storage_count = STORAGE_COUNT - 1,
        .immortal_objects = data->immortal_table,
        .immortal_object_count = 1,
        .initialization_units = data->unit_table,
        .initialization_unit_count = 2,
        .type_registrations = data->types,
        .type_registration_count = 1,
        .callables = data->callable_table,
        .callable_count = 8,
        .safepoints = (const void *)data->empty};
    data->input[0] = &data->image;
    data->root = (ScoopRootEntryDescriptorV1){
        .prefix =
            prefix(SCOOP_ROOT_ENTRY_DESCRIPTOR_MAGIC_V1, sizeof data->root),
        .owner_cone_identity = {{1}},
        .callable_id = {{1}},
        .source_signature_fingerprint = {{1}},
        .gateway_callable_id = {{7}},
        .gateway_definition_fingerprint = {{7}},
        .failure_root = &data->storages[ROOT_FAILURE],
        .gateway = root_gateway};
}

void scoop_test_storage_dispose(ScoopStorageFixture *fixture) {
    assert(munmap(fixture->data, fixture->size + fixture->page) == 0);
    assert(munmap(fixture->state, fixture->page) == 0);
}
