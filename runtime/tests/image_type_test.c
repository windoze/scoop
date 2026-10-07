#include "no_core.h"
#include <assert.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/wait.h>
#include <unistd.h>

#include "../src/image/internal.h"
#include "../src/value_shape.h"

typedef struct TypeMetadata {
    ScoopImageDescriptorV1 image;
    const ScoopImageDescriptorV1 *input[1];
    ScoopRootEntryDescriptorV1 root;
    uint64_t empty[1];
    char text[8];
    ScoopTypeRegistrationDescriptorV1 types[4];
    const ScoopTypeRegistrationDescriptorV1 *type_table[4];
    _Alignas(8) uint8_t
        descriptors[4][sizeof(ScoopTypeDescriptor) + sizeof(void *)];
    uint64_t object_scan[5], inline_scan[2];
    ScoopItableEntryV1 itable;
    ScoopCallableRegistrationDescriptorV1 callables[3];
    const ScoopCallableRegistrationDescriptorV1 *callable_table[3];
    ScoopSafepointRegistrationDescriptorV1 sites[2];
    const ScoopSafepointRegistrationDescriptorV1 *site_table[2];
} TypeMetadata;

typedef struct Fixture {
    TypeMetadata *data;
    size_t size;
    ScoopPlatformImageRange ranges[2];
    ScoopPlatformMetadataImages loaded;
} Fixture;

static void body0(void) {}
static void body1(void) {}
static void release0(void *object) { (void)object; }

static ScoopTypeDescriptor *td(TypeMetadata *data, size_t index) {
    return (ScoopTypeDescriptor *)data->descriptors[index];
}

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

static void setup(Fixture *fixture) {
    size_t page = (size_t)sysconf(_SC_PAGESIZE);
    fixture->size = (sizeof(TypeMetadata) + page - 1) / page * page;
    TypeMetadata *data =
        mmap(NULL, fixture->size + page, PROT_READ | PROT_WRITE,
             MAP_PRIVATE | MAP_ANON, -1, 0);
    assert(data != MAP_FAILED);
    fixture->data = data;
    assert(mprotect((uint8_t *)data + fixture->size, page, PROT_NONE) == 0);
    ScoopCallableAddressV1 bodies[] = {body0, body1,
                                       (ScoopCallableAddressV1)release0};
    uintptr_t low = (uintptr_t)bodies[0], high = low;
    for (size_t index = 0; index < 3; index++) {
        uintptr_t address = (uintptr_t)bodies[index];
        if (address < low)
            low = address;
        if (address > high)
            high = address;
    }
    fixture->ranges[0] = (ScoopPlatformImageRange){
        (uintptr_t)data, (uintptr_t)data + fixture->size, SCOOP_IMAGE_READ};
    fixture->ranges[1] = (ScoopPlatformImageRange){
        low, high + 4, SCOOP_IMAGE_READ | SCOOP_IMAGE_EXECUTE};
    qsort(fixture->ranges, 2, sizeof *fixture->ranges, range_order);
    fixture->loaded = (ScoopPlatformMetadataImages){.ranges = fixture->ranges,
                                                    .range_count = 2};
    strcpy(data->text, "test");
    ScoopByteSpanV1 name = {(const uint8_t *)data->text, 4};
    data->image = (ScoopImageDescriptorV1){
        .prefix = prefix(SCOOP_IMAGE_DESCRIPTOR_MAGIC_V1, sizeof data->image),
        .cone = {name, name, name, {{1}}},
        .runtime_image_fingerprint = {{1}},
        .dependencies = (const void *)data->empty,
        .static_storages = (const void *)data->empty,
        .immortal_objects = (const void *)data->empty,
        .initialization_units = (const void *)data->empty,
        .type_registrations = data->type_table,
        .type_registration_count = 4,
        .callables = data->callable_table,
        .callable_count = 3,
        .safepoints = data->site_table,
        .safepoint_count = 2};
    data->input[0] = &data->image;
    data->root = (ScoopRootEntryDescriptorV1){
        .prefix =
            prefix(SCOOP_ROOT_ENTRY_DESCRIPTOR_MAGIC_V1, sizeof data->root),
        .owner_cone_identity = {{1}},
        .callable_id = {{1}},
        .source_signature_fingerprint = {{1}},
        .gateway_callable_id = {{2}},
        .gateway_definition_fingerprint = {{2}}};
    for (size_t index = 0; index < 4; index++) {
        data->types[index] = (ScoopTypeRegistrationDescriptorV1){
            .prefix = prefix(SCOOP_TYPE_REGISTRATION_DESCRIPTOR_MAGIC_V1,
                             sizeof data->types[index]),
            .registration = identity((uint8_t)(index + 1)),
            .runtime_type_id = index + 1,
            .descriptor = td(data, index),
            .descriptor_fingerprint = {{1}},
            .layout_fingerprint = {{1}}};
        data->type_table[index] = &data->types[index];
        *td(data, index) = (ScoopTypeDescriptor){
            .type_id = index + 1,
            .diagnostic_name = name,
            .instance_shape = {.instance_kind =
                                   SCOOP_TYPE_INSTANCE_ABSTRACT_REF_V1}};
    }
    data->object_scan[0] = 1;
    data->object_scan[1] = 16;
    data->inline_scan[0] = 1;
    td(data, 0)->instance_shape = (ScoopTypeInstanceShapeV1){
        .instance_kind = SCOOP_TYPE_INSTANCE_FIXED_OBJECT_V1,
        .minimum_size = 24,
        .instance_alignment = 8};
    td(data, 0)->object_scan = data->object_scan;
    td(data, 0)->parent = td(data, 2);
    td(data, 0)->itables = &data->itable;
    td(data, 0)->itable_count = 1;
    data->itable = (ScoopItableEntryV1){td(data, 3), (const void *)data->empty};
    td(data, 1)->instance_shape = (ScoopTypeInstanceShapeV1){
        .instance_kind = SCOOP_TYPE_INSTANCE_BOXED_VALUE_V1,
        .inline_storage_kind = SCOOP_INLINE_STORAGE_INLINE_V1,
        .minimum_size = 24,
        .instance_alignment = 8,
        .inline_offset = 16,
        .inline_size = 8,
        .inline_alignment = 8,
        .inline_scan = data->inline_scan};
    td(data, 1)->object_scan = data->object_scan;
    td(data, 2)->relation_kind = 1;
    td(data, 2)->related_type_count = 1;
    td(data, 2)->related_types[0] = td(data, 0);
    td(data, 3)->relation_kind = 3;
    for (size_t index = 0; index < 3; index++) {
        data->callables[index] = (ScoopCallableRegistrationDescriptorV1){
            .prefix = prefix(SCOOP_CALLABLE_REGISTRATION_DESCRIPTOR_MAGIC_V1,
                             sizeof data->callables[index]),
            .registration = identity((uint8_t)(index + 1)),
            .body_definition_fingerprint = {{(uint8_t)(index + 1)}},
            .entry = bodies[index]};
        data->callable_table[index] = &data->callables[index];
    }
    /* Fixed vector for the exact type ID {1, 0, ...} and body-v2 tag 5. */
    static const ScoopDigest256V1 release_id = {
        {0x57, 0x11, 0x36, 0xa6, 0x01, 0xe0, 0x19, 0x2d, 0x1c, 0x3e, 0x15,
         0x9c, 0x26, 0x77, 0x92, 0x76, 0x9b, 0x7b, 0x8a, 0xab, 0xa6, 0x7c,
         0x73, 0xf7, 0x48, 0x89, 0x16, 0x8e, 0x21, 0x0b, 0xec, 0x53}};
    td(data, 0)->release_hook = release0;
    data->callables[2].registration.semantic_id = release_id;
    for (size_t index = 0; index < 2; index++) {
        data->sites[index] = (ScoopSafepointRegistrationDescriptorV1){
            .prefix = prefix(SCOOP_SAFEPOINT_REGISTRATION_DESCRIPTOR_MAGIC_V1,
                             sizeof data->sites[index]),
            .registration = identity((uint8_t)(index + 1)),
            .safepoint_id = index + 1,
            .site_role = SCOOP_SAFEPOINT_MANAGED_POLL_V1,
            .owner_callable_id = {{1}},
            .normalized_stackmap_fingerprint = {{1}}};
        data->site_table[index] = &data->sites[index];
    }
}

static const char *corrupt(Fixture *fixture, unsigned test) {
    TypeMetadata *data = fixture->data;
    const void *guard = (const uint8_t *)data + fixture->size;
    switch (test) {
    case 0:
        data->types[1].runtime_type_id = td(data, 1)->type_id = 1;
        return "64-bit runtime type ID collision";
    case 1:
        data->types[1].descriptor = td(data, 0);
        data->types[1].runtime_type_id = 1;
        return "different exact types share a TypeDescriptor address";
    case 2:
        td(data, 0)->parent = guard;
        return "unregistered TypeDescriptor";
    case 3:
        data->types[0].descriptor = guard;
        return "TypeDescriptor header range";
    case 4:
        td(data, 0)->related_type_count = UINT32_MAX;
        return "related types range";
    case 5:
        td(data, 0)->object_scan = guard;
        return "scan header range";
    case 6:
        data->object_scan[0] = SCOOP_REFS_ARRAY;
        data->object_scan[1] = 16;
        data->object_scan[2] = 24;
        data->object_scan[3] = 8;
        data->object_scan[4] = (uintptr_t)data->object_scan;
        return "cyclic scan child";
    case 7:
        data->object_scan[1] = 24;
        return "reference scan exceeds its storage";
    case 8:
        data->object_scan[1] = 17;
        return "reference scan offsets are not canonical";
    case 9:
        td(data, 0)->parent = td(data, 0);
        return "cyclic type inheritance";
    case 10:
        td(data, 3)->related_type_count = 1;
        td(data, 3)->related_types[0] = td(data, 3);
        return "cyclic type inheritance";
    case 11:
        data->callables[1].entry = body0;
        return "different callable bodies share an entry address";
    case 12:
        data->sites[1].safepoint_id = 1;
        return "64-bit safepoint ID collision";
    case 13:
        data->sites[1].owner_callable_id.bytes[0] = 200;
        return "owner callable identity or producer";
    case 14:
        data->sites[0].site_role = 0;
        return "site ID, role";
    case 15:
        data->itable.interface = guard;
        return "unregistered TypeDescriptor";
    case 16:
        data->itable.slots = guard;
        return "itable slots range";
    case 17:
        td(data, 0)->itables = guard;
        return "itable entries range";
    case 18:
        data->types[0].reserved_zero = 1;
        return "reserved field";
    case 19:
        data->callables[0].entry = (ScoopCallableAddressV1)(uintptr_t)data;
        return "entry executable range";
    case 20:
        td(data, 1)->instance_shape = (ScoopTypeInstanceShapeV1){
            .instance_kind = SCOOP_TYPE_INSTANCE_FIXED_OBJECT_V1,
            .minimum_size = 16,
            .instance_alignment = 8};
        return "reference scan exceeds its storage";
    case 21:
        td(data, 3)->related_type_count = 1;
        td(data, 3)->related_types[0] = NULL;
        return "unregistered TypeDescriptor";
    case 22:
        data->callables[2].registration.semantic_id.bytes[0] ^= 0xff;
        return "release hook callable registration";
    case 23:
        td(data, 0)->release_hook = (ScoopReleaseHookV1)body0;
        return "release hook entry disagrees with exact owner";
    case 24:
        td(data, 1)->release_hook = release0;
        return "release hook requires a fixed object";
    case 25:
        data->types[0].registration.semantic_id.bytes[0] = 250;
        return "release hook callable registration";
    case 26:
        data->callables[2].entry = (ScoopCallableAddressV1)(uintptr_t)data;
        return "entry executable range";
    default:
        abort();
    }
}

static void check(Fixture *fixture) {
    assert(mprotect(fixture->data, fixture->size, PROT_READ) == 0);
    ScoopImageRegistry *registry = scoop_image_collect(
        &fixture->loaded, fixture->data->input, 1, &fixture->data->root);
    scoop_image_validate_code_and_types(registry);
    for (size_t index = 0; index < 4; index++) {
        assert(scoop_image_type(registry, td(fixture->data, index)) ==
               &fixture->data->types[index]);
    }
    assert(scoop_image_type(registry, (const void *)fixture->data->empty) ==
           NULL);
    scoop_image_registry_dispose(registry);
}

static void negative(unsigned test) {
    Fixture fixture = {0};
    setup(&fixture);
    const char *expected = corrupt(&fixture, test);
    int output[2];
    assert(pipe(output) == 0);
    pid_t child = fork();
    assert(child >= 0);
    if (child == 0) {
        close(output[0]);
        assert(dup2(output[1], STDERR_FILENO) >= 0);
        close(output[1]);
        scoop_test_disable_core_dumps();
        check(&fixture);
        _exit(0);
    }
    close(output[1]);
    char message[1024] = {0};
    size_t used = 0;
    ssize_t amount;
    while ((amount = read(output[0], message + used,
                          sizeof message - used - 1)) > 0) {
        used += (size_t)amount;
    }
    close(output[0]);
    int status;
    assert(waitpid(child, &status, 0) == child);
    if (!WIFSIGNALED(status) || WTERMSIG(status) != SIGABRT ||
        strstr(message, expected) == NULL) {
        fprintf(stderr, "case %u expected %s, status %d: %s", test, expected,
                status, message);
        abort();
    }
    assert(munmap(fixture.data, fixture.size + (size_t)sysconf(_SC_PAGESIZE)) ==
           0);
}

int main(void) {
    for (unsigned empty = 0; empty < 2; empty++) {
        Fixture fixture = {0};
        setup(&fixture);
        if (empty)
            fixture.data->itable.slots = NULL;
        check(&fixture);
        assert(munmap(fixture.data,
                      fixture.size + (size_t)sysconf(_SC_PAGESIZE)) == 0);
    }
    for (unsigned test = 0; test < 27; test++)
        negative(test);
    puts("image type and scan tests passed");
}
