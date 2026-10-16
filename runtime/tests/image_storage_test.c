#include "no_core.h"
#include <assert.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/wait.h>
#include <unistd.h>

#include "image_storage_fixture.h"

static void aggregate_value(ScoopStorageFixture *fixture) {
    StorageMetadata *data = fixture->data;
    data->image.static_storage_count = STORAGE_COUNT;
    ScoopStaticStorageDescriptorV1 *storage = &data->storages[EXTRA_STORAGE];
    storage->writable_base = fixture->state->aggregate;
    storage->byte_size = storage->allocation_extent = 32;
    storage->initial_state_kind = SCOOP_STATIC_INITIAL_ENCODED_VALUE_V1;
    storage->initial_template =
        (ScoopByteSpanV1){data->templates[EXTRA_STORAGE], 32};
    storage->scan_kind = SCOOP_STATIC_SCAN_RECURSIVE_V1;
    storage->scan_program = data->scan_nodes[2];
    data->scan_nodes[0][0] = 1;
    data->scan_nodes[0][1] = 24;
    uint64_t array[] = {SCOOP_REFS_ARRAY, 0, 8, 8,
                        (uintptr_t)data->reference_scan};
    memcpy(data->scan_nodes[1], array, sizeof array);
    uint64_t sequence[] = {SCOOP_REFS_SEQUENCE, 2,
                           (uintptr_t)data->scan_nodes[0],
                           (uintptr_t)data->scan_nodes[1]};
    memcpy(data->scan_nodes[2], sequence, sizeof sequence);
    fixture->state->aggregate[0] = 2;
    fixture->state->aggregate[1] = (uintptr_t)&data->string_object;
    fixture->state->aggregate[3] = (uintptr_t)&data->string_object;
    data->relocations[2] =
        (ScoopStaticImmortalRelocationV1){8, &data->immortals[0]};
    data->relocations[3] =
        (ScoopStaticImmortalRelocationV1){24, &data->immortals[0]};
    storage->initial_relocations = &data->relocations[2];
    storage->initial_relocation_count = 2;
}

static ScoopImageRegistry *check(ScoopStorageFixture *fixture) {
    ScoopImageRegistry *registry = scoop_image_collect(
        &fixture->loaded, fixture->data->input, 1, &fixture->data->root);
    scoop_image_validate_code_and_types(registry);
    scoop_image_validate_storage_and_units(registry);
    return registry;
}

static void positive(bool both_eager) {
    ScoopStorageFixture fixture = {0};
    scoop_test_storage_setup(&fixture, both_eager);
    /* The compiler supplies the initial bytes; registration collects roots. */
    fixture.data->image.static_storage_count++;
    fixture.data->storages[EXTRA_STORAGE].initial_state_kind =
        SCOOP_STATIC_INITIAL_ENCODED_VALUE_V1;
    fixture.data->storages[EXTRA_STORAGE].initial_template =
        (ScoopByteSpanV1){fixture.data->templates[EXTRA_STORAGE], 8};
    fixture.state->slots[EXTRA_STORAGE] = 123;
    ScoopImageRegistry *registry = check(&fixture);
    assert(registry->static_root_count == 4);
    for (size_t index = 0; index < registry->static_root_count; index++) {
        assert(registry->static_roots[index]->byte_size == 8);
        assert(registry->static_roots[index] !=
               &fixture.data->storages[ZST_TOKEN]);
    }
    assert(registry->eager_unit_count == (both_eager ? 2 : 1));
    assert(registry->eager_units[0] ==
           &fixture.data->units[both_eager ? 1 : 0]);
    if (both_eager)
        assert(registry->eager_units[1] == &fixture.data->units[0]);
    assert(scoop_image_immortal(registry, &fixture.data->string_object) ==
           &fixture.data->immortals[0]);
    assert(scoop_image_immortal(
               registry, (const uint8_t *)&fixture.data->string_object + 8) ==
           NULL);
    /* Initial states are not rechecked during indexed lookups. */
    fixture.state->cells[0].state = 2;
    fixture.state->slots[PROPERTY_A] = 7;
    assert(scoop_record_by_address(registry, SCOOP_RECORD_UNIT,
                                   &fixture.data->units[0]) != NULL);
    scoop_image_registry_dispose(registry);
    scoop_test_storage_dispose(&fixture);
}

static void positive_aggregate(void) {
    ScoopStorageFixture fixture = {0};
    scoop_test_storage_setup(&fixture, false);
    aggregate_value(&fixture);
    ScoopImageRegistry *registry = check(&fixture);
    assert(registry->static_root_count == 5);
    scoop_image_registry_dispose(registry);
    scoop_test_storage_dispose(&fixture);
}

static const char *corrupt(ScoopStorageFixture *fixture, unsigned test) {
    StorageMetadata *data = fixture->data;
    StorageState *state = fixture->state;
    const void *guard = (const uint8_t *)data + fixture->size;
    switch (test) {
    case 0:
        data->units[1].failure_root = data->units[0].failure_root;
        return "shared storage role";
    case 1:
        data->root.failure_root = data->units[0].failure_root;
        return "shared storage role";
    case 2:
        data->units[0].storage = data->units[0].failure_root;
        return "shared storage role";
    case 3:
        data->units[1].cell = data->units[0].cell;
        return "overlapping storage or cell";
    case 4:
        data->units[0].cell = (void *)&state->slots[PROPERTY_A];
        return "overlapping storage or cell";
    case 5:
        data->storages[PROPERTY_B].writable_base = &state->slots[PROPERTY_A];
        return "overlapping storage or cell";
    case 6:
        data->storages[ZST_TOKEN].writable_base = &state->slots[PROPERTY_A];
        return "overlapping storage or cell";
    case 7:
        data->string_object.header.td = guard;
        return "object header TypeDescriptor";
    case 8:
        data->immortals[0].object_size = 24;
        return "object count, alignment or side-metadata size";
    case 9:
        data->string_object.length = UINT64_MAX;
        return "physical count exceeds INT64_MAX";
    case 10: {
        ScoopTypeDescriptor *td = (ScoopTypeDescriptor *)data->td_bytes;
        td->instance_shape = (ScoopTypeInstanceShapeV1){
            .instance_kind = SCOOP_TYPE_INSTANCE_FIXED_OBJECT_V1,
            .minimum_size = 24,
            .instance_alignment = 8};
        td->object_scan = data->reference_scan;
        return "object size, alignment or reference scan";
    }
    case 11:
        data->image.static_storage_count++;
        return "zeroed storage has no initialization or failure role";
    case 12:
        data->units[0].storage = &data->storages[ZST_TOKEN];
        return "shared storage role, initial state";
    case 13:
        data->units[0].schedule_kind = 0;
        return "unknown schedule kind";
    case 14:
        data->units[1].startup_gateway = data->units[0].startup_gateway;
        return "lazy unit has a startup gateway";
    case 15:
        data->units[0].storage = guard;
        return "unregistered storage";
    case 16:
        data->storages[PROPERTY_A].scan_program = data->reference_scan;
        return "None scan has references";
    case 17:
        data->storages[PROPERTY_A].scan_program = guard;
        return "scan header range";
    case 18:
        data->storages[FAILURE_A].required_alignment = 4;
        return "dedicated reference slot";
    case 19:
        data->storages[PROPERTY_A].byte_size =
            data->storages[PROPERTY_A].allocation_extent = UINT64_MAX;
        return "writable allocation range";
    case 20:
        data->root.callable_id.bytes[0] = 200;
        return "source callable owner";
    case 21:
        data->immortals[1] = data->immortals[0];
        data->immortals[1].registration.semantic_id.bytes[0] = 2;
        data->immortal_table[1] = &data->immortals[1];
        data->image.immortal_object_count = 2;
        return "overlapping immortal ranges";
    case 22:
        aggregate_value(fixture);
        data->scan_nodes[1][4] = 0;
        return "scan header range";
    default:
        abort();
    }
}

static void negative(unsigned test) {
    ScoopStorageFixture fixture = {0};
    scoop_test_storage_setup(&fixture, false);
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
        scoop_image_registry_dispose(check(&fixture));
        _exit(0);
    }
    close(output[1]);
    char message[1024] = {0};
    size_t used = 0;
    ssize_t amount;
    while ((amount =
                read(output[0], message + used, sizeof message - used - 1)) > 0)
        used += (size_t)amount;
    close(output[0]);
    int status;
    assert(waitpid(child, &status, 0) == child);
    if (!WIFSIGNALED(status) || WTERMSIG(status) != SIGABRT ||
        strstr(message, expected) == NULL) {
        fprintf(stderr, "case %u expected %s, status %d: %s", test, expected,
                status, message);
        abort();
    }
    scoop_test_storage_dispose(&fixture);
}

int main(void) {
    positive(false);
    positive(true);
    positive_aggregate();
    for (unsigned test = 0; test < 23; test++)
        negative(test);
    puts("image storage and initialization tests passed");
}
