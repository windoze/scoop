#include <assert.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/resource.h>
#include <sys/wait.h>
#include <unistd.h>

#include "image_stackmap_fixture.h"

static ScoopImageRegistry *check(ScoopStackmapFixture *fixture) {
    assert(mprotect(fixture->storage.data, fixture->storage.size, PROT_READ) == 0);
    ScoopImageRegistry *registry =
        scoop_image_collect(&fixture->storage.loaded, fixture->storage.data->input, 1,
                            &fixture->storage.data->root);
    scoop_image_validate_code_and_types(registry);
    scoop_image_stackmaps(registry, &scoop_darwin_aarch64_managed_frame_ops);
    return registry;
}

static void positive(void) {
    scoop_test_stackmap_rust_vector();
    ScoopStackmapFixture fixture = {0};
    scoop_test_stackmap_setup(&fixture);
    ScoopImageRegistry *registry = check(&fixture);
    assert(registry->stackmaps.record_count == 2);
    uintptr_t pc = (uintptr_t)fixture.storage.data->callables[1].entry + 8;
    const ScoopStackMapRecord *record = scoop_stackmap_lookup(&registry->stackmaps, pc);
    assert(record != NULL && record->safepoint_id == 11 && record->root_count == 1);
    assert(record->live_out_count == 1 && record->live_outs[0].dwarf_register == 19);
    assert(record->live_outs[0].size == 8 && record->instruction_offset == 8);
    assert(record->section_offset == fixture.blobs[0].record);
    assert(scoop_stackmap_lookup(&registry->stackmaps, pc + 1) == NULL);
    scoop_image_registry_dispose(registry);
    scoop_test_storage_dispose(&fixture.storage);
}

static const char *corrupt(ScoopStackmapFixture *fixture, unsigned test) {
    StorageMetadata *data = fixture->storage.data;
    BlobOffsets first = fixture->blobs[0], second = fixture->blobs[1],
                third = fixture->blobs[2];
    switch (test) {
    case 0:
        fixture->image.section_size = first.end;
        return "safepoint registration has no raw record";
    case 1:
        scoop_test_stackmap_write(fixture, first.record, 999, 8);
        return "raw record has no safepoint registration";
    case 2:
        data->sites[2] = data->sites[1];
        data->sites[2].registration.semantic_id.bytes[0]++;
        data->sites[2].safepoint_id++;
        data->site_table[2] = &data->sites[2];
        data->image.safepoint_count++;
        return "safepoint registration has no raw record";
    case 3:
        scoop_test_stackmap_write(fixture, first.start + 16,
                                  (uintptr_t)data->callables[0].entry, 8);
        return "owner entry address or root count";
    case 4:
        scoop_test_stackmap_write(fixture, first.record + 8, 12, 4);
        return "normalized fingerprint mismatch";
    case 5:
        scoop_test_stackmap_write(fixture, first.locations + 36 + 8, 20, 4);
        scoop_test_stackmap_write(fixture, first.locations + 48 + 8, 20, 4);
        return "normalized fingerprint mismatch";
    case 6:
        scoop_test_stackmap_write(fixture, first.live_outs, 20, 2);
        return "normalized fingerprint mismatch";
    case 7:
        data->sites[0].registration.linkage_kind = SCOOP_REGISTRATION_LINKAGE_STRONG_V1;
        data->sites[0].registration.odr_group_id = (ScoopDigest256V1){0};
        data->sites[0].registration.odr_member_id = (ScoopDigest256V1){0};
        data->callables[1].registration.linkage_kind =
            SCOOP_REGISTRATION_LINKAGE_STRONG_V1;
        data->callables[1].registration.odr_group_id = (ScoopDigest256V1){0};
        data->callables[1].registration.odr_member_id = (ScoopDigest256V1){0};
        return "duplicate Strong raw record";
    case 8:
        scoop_test_stackmap_write(fixture, third.record + 8, 12, 4);
        return "normalized fingerprint mismatch";
    case 9:
        data->sites[1].owner_callable_id = data->sites[0].owner_callable_id;
        data->sites[1].registration.linkage_kind = SCOOP_REGISTRATION_LINKAGE_ODR_V1;
        data->sites[1].registration.odr_group_id.bytes[0] = 55;
        data->sites[1].registration.odr_member_id.bytes[0] = 78;
        scoop_test_stackmap_write(fixture, second.start + 16,
                                  (uintptr_t)data->callables[1].entry, 8);
        scoop_test_stackmap_write(fixture, second.record + 8, 8, 4);
        scoop_test_stackmap_fingerprint(&data->sites[1], 8);
        return "duplicate statepoint return PC";
    case 10:
        data->sites[0].normalized_stackmap_fingerprint.bytes[0] ^= 1;
        return "normalized fingerprint mismatch";
    case 11:
        data->sites[0].root_pair_count = 0;
        return "owner entry address or root count";
    case 12:
        scoop_test_stackmap_write(fixture, first.locations + 4, 1, 2);
        return "unknown stack map location kind";
    case 13:
        scoop_test_stackmap_write(fixture, third.start + 40, 42, 8);
        return "normalized fingerprint mismatch";
    case 14:
        fixture->image.section = (const uint8_t *)data + fixture->storage.size;
        return "complete section readonly range";
    case 15:
        scoop_test_stackmap_write(fixture, first.start + 12, UINT32_MAX, 4);
        return "function and record counts disagree";
    case 16:
        fixture->image.section_size--;
        return "truncated stack map section";
    case 17:
        scoop_test_stackmap_write(fixture, first.start + 4, UINT32_MAX, 4);
        return "truncated stack map section";
    case 18:
        scoop_test_stackmap_write(fixture, first.locations + 36 + 4, 0, 2);
        return "invalid managed frame or root location";
    case 19:
        scoop_test_stackmap_write(fixture, first.locations + 36 + 4, 29, 2);
        scoop_test_stackmap_write(fixture, first.locations + 48 + 4, 29, 2);
        scoop_test_stackmap_write(fixture, first.locations + 36 + 8, INT32_MAX, 4);
        scoop_test_stackmap_write(fixture, first.locations + 48 + 8, INT32_MAX, 4);
        scoop_test_stackmap_write(fixture, first.start + 24,
                                  (uint64_t)INT64_MAX & ~UINT64_C(15), 8);
        return "invalid managed frame or root location";
    default:
        abort();
    }
}

static void negative(unsigned test) {
    ScoopStackmapFixture fixture = {0};
    scoop_test_stackmap_setup(&fixture);
    const char *expected = corrupt(&fixture, test);
    int output[2];
    assert(pipe(output) == 0);
    pid_t child = fork();
    assert(child >= 0);
    if (child == 0) {
        close(output[0]);
        assert(dup2(output[1], STDERR_FILENO) >= 0);
        close(output[1]);
        struct rlimit limit = {0, 0};
        assert(setrlimit(RLIMIT_CORE, &limit) == 0);
        scoop_image_registry_dispose(check(&fixture));
        _exit(0);
    }
    close(output[1]);
    char message[2048] = {0};
    size_t used = 0;
    ssize_t amount;
    while ((amount = read(output[0], message + used, sizeof message - used - 1)) > 0)
        used += (size_t)amount;
    close(output[0]);
    int status;
    assert(waitpid(child, &status, 0) == child);
    if (!WIFSIGNALED(status) || WTERMSIG(status) != SIGABRT ||
        strstr(message, expected) == NULL) {
        fprintf(stderr, "case %u expected %s, status %d: %s", test, expected, status,
                message);
        abort();
    }
    scoop_test_storage_dispose(&fixture.storage);
}

int main(void) {
    positive();
    for (unsigned test = 0; test < 20; test++)
        negative(test);
    puts("image stackmap join tests passed");
}
