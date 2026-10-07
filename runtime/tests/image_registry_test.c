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

enum { ROOT, Z, B, A, C, IMAGE_COUNT };

typedef struct Metadata {
    ScoopImageDescriptorV1 images[IMAGE_COUNT];
    const ScoopImageDescriptorV1 *input[IMAGE_COUNT];
    ScoopRootEntryDescriptorV1 root;
    ScoopDigest256V1 dependencies[IMAGE_COUNT][IMAGE_COUNT];
    const void *empty[1];
    char group[8], names[IMAGE_COUNT][8], version[8];
    ScoopCallableRegistrationDescriptorV1 callables[4];
    const ScoopCallableRegistrationDescriptorV1
        *callable_tables[IMAGE_COUNT][3];
    ScoopStaticStorageDescriptorV1 storage;
    const ScoopStaticStorageDescriptorV1 *storages[1];
    ScoopImmortalObjectDescriptorV1 immortal;
    const ScoopImmortalObjectDescriptorV1 *immortals[1];
    ScoopInitializationUnitDescriptorV1 unit;
    const ScoopInitializationUnitDescriptorV1 *units[1];
    ScoopTypeRegistrationDescriptorV1 type;
    const ScoopTypeRegistrationDescriptorV1 *types[1];
    ScoopSafepointRegistrationDescriptorV1 site;
    const ScoopSafepointRegistrationDescriptorV1 *sites[1];
} Metadata;

typedef struct Fixture {
    Metadata *metadata;
    size_t size;
    ScoopPlatformImageRange range;
    ScoopPlatformMetadataImages loaded;
} Fixture;

static ScoopDescriptorPrefixV1 prefix(uint64_t magic, size_t size) {
    return (ScoopDescriptorPrefixV1){
        magic, SCOOP_RUNTIME_METADATA_ABI_VERSION_V1, (uint32_t)size};
}

static ScoopRegistrationIdentityV1 identity(uint8_t id, bool odr) {
    ScoopRegistrationIdentityV1 result = {
        .linkage_kind = odr ? SCOOP_REGISTRATION_LINKAGE_ODR_V1
                            : SCOOP_REGISTRATION_LINKAGE_STRONG_V1,
        .semantic_id = {{id}}};
    if (odr) {
        result.odr_group_id.bytes[0] = 42;
        result.odr_member_id.bytes[0] = id;
    }
    return result;
}

static ScoopByteSpanV1 bytes(char *text) {
    return (ScoopByteSpanV1){(const uint8_t *)text, strlen(text)};
}

static void setup(Fixture *fixture) {
    size_t page = (size_t)sysconf(_SC_PAGESIZE);
    fixture->size = (sizeof(Metadata) + page - 1) / page * page;
    fixture->metadata = mmap(NULL, fixture->size + page, PROT_READ | PROT_WRITE,
                             MAP_PRIVATE | MAP_ANON, -1, 0);
    assert(fixture->metadata != MAP_FAILED);
    assert(mprotect((uint8_t *)fixture->metadata + fixture->size, page,
                    PROT_NONE) == 0);
    Metadata *data = fixture->metadata;
    fixture->range = (ScoopPlatformImageRange){
        (uintptr_t)data, (uintptr_t)data + fixture->size, SCOOP_IMAGE_READ};
    fixture->loaded = (ScoopPlatformMetadataImages){.ranges = &fixture->range,
                                                    .range_count = 1};
    strcpy(data->group, "test");
    strcpy(data->version, "1.0.0");
    const char *names[] = {"root", "z", "b", "a", "c"};
    for (size_t i = 0; i < IMAGE_COUNT; i++) {
        strcpy(data->names[i], names[i]);
        ScoopImageDescriptorV1 *image = &data->images[i];
        image->prefix = prefix(SCOOP_IMAGE_DESCRIPTOR_MAGIC_V1, sizeof *image);
        image->cone = (ScoopConeRecordV1){bytes(data->group),
                                          bytes(data->names[i]),
                                          bytes(data->version),
                                          {{(uint8_t)(i + 1)}}};
        image->runtime_image_fingerprint.bytes[0] = 1;
        image->dependencies = data->dependencies[i];
        image->static_storages =
            (const ScoopStaticStorageDescriptorV1 *const *)data->empty;
        image->immortal_objects =
            (const ScoopImmortalObjectDescriptorV1 *const *)data->empty;
        image->initialization_units =
            (const ScoopInitializationUnitDescriptorV1 *const *)data->empty;
        image->type_registrations =
            (const ScoopTypeRegistrationDescriptorV1 *const *)data->empty;
        image->safepoints =
            (const ScoopSafepointRegistrationDescriptorV1 *const *)data->empty;
        image->callables = data->callable_tables[i];
        data->input[i] = image;
    }
    data->dependencies[ROOT][0] = data->images[Z].cone.identity;
    data->dependencies[ROOT][1] = data->images[C].cone.identity;
    data->images[ROOT].dependency_count = 2;
    data->dependencies[C][0] = data->images[B].cone.identity;
    data->images[C].dependency_count = 1;
    data->dependencies[B][0] = data->images[A].cone.identity;
    data->images[B].dependency_count = 1;
    data->root = (ScoopRootEntryDescriptorV1){
        .prefix =
            prefix(SCOOP_ROOT_ENTRY_DESCRIPTOR_MAGIC_V1, sizeof data->root),
        .owner_cone_identity = data->images[ROOT].cone.identity,
        .callable_id = {{1}},
        .source_signature_fingerprint = {{2}},
        .gateway_callable_id = {{3}},
        .gateway_definition_fingerprint = {{4}}};
    for (size_t i = 0; i < 4; i++) {
        data->callables[i].prefix =
            prefix(SCOOP_CALLABLE_REGISTRATION_DESCRIPTOR_MAGIC_V1,
                   sizeof data->callables[i]);
        data->callables[i].registration = identity((uint8_t)(i + 1), true);
    }
    data->callable_tables[B][0] = &data->callables[0];
    data->callable_tables[B][1] = &data->callables[1];
    data->images[B].callable_count = 2;
    data->callable_tables[C][0] = &data->callables[0];
    data->callable_tables[C][1] = &data->callables[2];
    data->images[C].callable_count = 2;
#define SINGLE_RECORD(field, table, image_field, count_field, magic)           \
    data->field.prefix = prefix(magic, sizeof data->field);                    \
    data->field.registration = identity(1, false);                             \
    data->table[0] = &data->field;                                             \
    data->images[ROOT].image_field = data->table;                              \
    data->images[ROOT].count_field = 1
    SINGLE_RECORD(storage, storages, static_storages, static_storage_count,
                  SCOOP_STATIC_STORAGE_DESCRIPTOR_MAGIC_V1);
    SINGLE_RECORD(immortal, immortals, immortal_objects, immortal_object_count,
                  SCOOP_IMMORTAL_OBJECT_DESCRIPTOR_MAGIC_V1);
    SINGLE_RECORD(unit, units, initialization_units, initialization_unit_count,
                  SCOOP_INITIALIZATION_UNIT_DESCRIPTOR_MAGIC_V1);
    SINGLE_RECORD(type, types, type_registrations, type_registration_count,
                  SCOOP_TYPE_REGISTRATION_DESCRIPTOR_MAGIC_V1);
    SINGLE_RECORD(site, sites, safepoints, safepoint_count,
                  SCOOP_SAFEPOINT_REGISTRATION_DESCRIPTOR_MAGIC_V1);
#undef SINGLE_RECORD
}

static void freeze(Fixture *fixture) {
    assert(mprotect(fixture->metadata, fixture->size, PROT_READ) == 0);
}

static ScoopImageRegistry *collect(Fixture *fixture) {
    return scoop_image_collect(&fixture->loaded, fixture->metadata->input,
                               IMAGE_COUNT, &fixture->metadata->root);
}

static void positive(bool reversed) {
    Fixture fixture = {0};
    setup(&fixture);
    Metadata *data = fixture.metadata;
    if (reversed) {
        for (size_t i = 0; i < IMAGE_COUNT; i++) {
            data->input[i] = &data->images[IMAGE_COUNT - i - 1];
        }
    }
    freeze(&fixture);
    ScoopImageRegistry *registry = collect(&fixture);
    const size_t order[] = {A, B, C, Z, ROOT};
    for (size_t i = 0; i < IMAGE_COUNT; i++) {
        assert(registry->image_order[i] == &data->images[order[i]]);
    }
    /* Equal bytes in different semantic ID namespaces are valid. */
    for (size_t kind = 0; kind < SCOOP_RECORD_KIND_COUNT; kind++) {
        assert(registry->tables[kind].count ==
               (kind == SCOOP_RECORD_CALLABLE ? 3 : 1));
        ScoopDigest256V1 id = {{1}};
        const ScoopRegisteredRecord *record =
            scoop_record_by_id(registry, kind, &id);
        assert(record != NULL);
        assert(scoop_record_by_address(registry, kind, record->record) ==
               record);
        id.bytes[0] = 250;
        assert(scoop_record_by_id(registry, kind, &id) == NULL);
        assert(scoop_record_by_address(registry, kind, data->empty) == NULL);
    }
    scoop_image_registry_dispose(registry);
    assert(munmap(data, fixture.size + (size_t)sysconf(_SC_PAGESIZE)) == 0);
}

static const char *corrupt(Fixture *fixture, unsigned test) {
    Metadata *data = fixture->metadata;
    switch (test) {
    case 0:
        data->root.prefix.abi_version--;
        return "prefix magic, ABI or exact size";
    case 1:
        data->input[0] =
            (const void *)((const uint8_t *)data + fixture->size - 8);
        return "prefix range";
    case 2:
        data->images[0].prefix.struct_size--;
        return "prefix magic, ABI or exact size";
    case 3:
        data->storage.registration.reserved_zero = 1;
        return "reserved field";
    case 4:
        fixture->range.permissions |= SCOOP_IMAGE_WRITE;
        return "prefix range";
    case 5:
        data->images[0].static_storages = NULL;
        return "producer pointer span";
    case 6:
        data->images[0].static_storage_count = UINT64_MAX;
        return "producer pointer span";
    case 7:
        data->images[0].dependencies = (const void *)(UINTPTR_MAX - 8);
        return "dependency span";
    case 8:
        data->dependencies[ROOT][0].bytes[0] = 200;
        return "dependency or root owner is absent";
    case 9:
        data->input[1] = data->input[0];
        return "duplicate identity";
    case 10:
        data->images[ROOT].dependency_count = 1;
        return "outside the root dependency closure";
    case 11:
        data->images[A].dependency_count = 1;
        data->dependencies[A][0] = data->images[C].cone.identity;
        return "cyclic image dependencies";
    case 12:
        data->dependencies[B][0] = data->images[B].cone.identity;
        return "image self dependency";
    case 13:
        data->images[A].cone.name = data->images[B].cone.name;
        return "duplicate group/name or multiple versions";
    case 14:
        data->images[Z].static_storages = data->storages;
        data->images[Z].static_storage_count = 1;
        return "duplicate Strong producer";
    case 15:
        data->callables[3] = data->callables[0];
        data->callable_tables[C][0] = &data->callables[3];
        return "ODR record address was not coalesced";
    case 16:
        data->storage.registration = data->callables[0].registration;
        return "group/member names different records or kinds";
    case 17:
        data->storage.registration.odr_group_id.bytes[0] = 1;
        return "linkage or ODR group/member fields";
    case 18:
        data->root.owner_cone_identity.bytes[0] = 200;
        return "dependency or root owner is absent";
    case 19:
        data->images[ROOT].dependency_count = 3;
        data->dependencies[ROOT][2] = data->dependencies[ROOT][0];
        return "duplicate image dependency";
    case 20:
        data->storages[0] =
            (const void *)((const uint8_t *)data + fixture->size);
        return "prefix range";
    case 21:
        data->images[ROOT].cone.name.data =
            (const void *)((const uint8_t *)data + fixture->size);
        return "name span";
    case 22:
        data->images[ROOT].static_storages =
            (const void *)((const uint8_t *)data->storages + 1);
        return "producer pointer span";
    default:
        abort();
    }
}

static void negative(unsigned test) {
    Fixture fixture = {0};
    setup(&fixture);
    const char *expected = corrupt(&fixture, test);
    freeze(&fixture);
    int output[2];
    assert(pipe(output) == 0);
    pid_t child = fork();
    assert(child >= 0);
    if (child == 0) {
        close(output[0]);
        assert(dup2(output[1], STDERR_FILENO) >= 0);
        close(output[1]);
        scoop_test_disable_core_dumps();
        scoop_image_registry_dispose(collect(&fixture));
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
        strstr(message, "scoop runtime metadata:") == NULL ||
        strstr(message, expected) == NULL) {
        fprintf(stderr, "case %u expected %s, status %d: %s", test, expected,
                status, message);
        abort();
    }
    assert(munmap(fixture.metadata,
                  fixture.size + (size_t)sysconf(_SC_PAGESIZE)) == 0);
}

int main(void) {
    positive(false);
    positive(true);
    for (unsigned test = 0; test < 23; test++) {
        negative(test);
    }
    puts("image registry collection tests passed");
}
