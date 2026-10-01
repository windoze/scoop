#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "internal.h"

_Noreturn void scoop_metadata_fatal(const ScoopMetadataCheck *check,
                                    const char *field) {
    fprintf(stderr, "scoop runtime metadata: %s", check ? check->kind : "registry");
    if (check != NULL && check->identity != NULL) {
        fputc('[', stderr);
        for (size_t index = 0; index < 32; index++) {
            fprintf(stderr, "%02x", check->identity->semantic_id.bytes[index]);
        }
        fputc(']', stderr);
    }
    fprintf(stderr, ": %s\n", field);
    abort();
}

void *scoop_metadata_allocate(size_t count, size_t size) {
    if (size == 0 || count > SIZE_MAX / size) {
        scoop_metadata_fatal(NULL, "native index size overflow");
    }
    void *result = calloc(count == 0 ? 1 : count, size);
    if (result == NULL) {
        scoop_metadata_fatal(NULL, "cannot allocate native index");
    }
    return result;
}

bool scoop_digest_zero(const ScoopDigest256V1 *digest) {
    static const ScoopDigest256V1 zero;
    return scoop_digest_equal(digest, &zero);
}

bool scoop_digest_equal(const ScoopDigest256V1 *left, const ScoopDigest256V1 *right) {
    return memcmp(left->bytes, right->bytes, sizeof left->bytes) == 0;
}

void scoop_metadata_readonly(const ScoopMetadataCheck *check, const void *pointer,
                             uint64_t count, size_t size, size_t alignment,
                             const char *field) {
    if (size == 0 || count > SIZE_MAX / size ||
        !scoop_image_range_contains(check->loaded, pointer, count * size, alignment,
                                    SCOOP_IMAGE_READ, SCOOP_IMAGE_WRITE)) {
        scoop_metadata_fatal(check, field);
    }
}

void scoop_metadata_writable(const ScoopMetadataCheck *check, void *pointer,
                             uint64_t size, uint64_t alignment, const char *field) {
    if (!scoop_image_range_contains(check->loaded, pointer, size, alignment,
                                    SCOOP_IMAGE_READ | SCOOP_IMAGE_WRITE,
                                    SCOOP_IMAGE_EXECUTE)) {
        scoop_metadata_fatal(check, field);
    }
}

void scoop_metadata_executable(const ScoopMetadataCheck *check,
                               ScoopCallableAddressV1 entry, const char *field) {
    if (!scoop_image_range_contains(check->loaded, (const void *)(uintptr_t)entry, 4, 4,
                                    SCOOP_IMAGE_READ | SCOOP_IMAGE_EXECUTE,
                                    SCOOP_IMAGE_WRITE)) {
        scoop_metadata_fatal(check, field);
    }
}

void scoop_metadata_prefix(const ScoopMetadataCheck *check, const void *record,
                           uint64_t magic, size_t size) {
    scoop_metadata_readonly(check, record, 1, sizeof(ScoopDescriptorPrefixV1), 8,
                            "prefix range");
    const ScoopDescriptorPrefixV1 *prefix = record;
    if (prefix->magic != magic ||
        prefix->abi_version != SCOOP_RUNTIME_METADATA_ABI_VERSION_V1 ||
        prefix->struct_size != size) {
        scoop_metadata_fatal(check, "prefix magic, ABI or exact size");
    }
    scoop_metadata_readonly(check, record, 1, size, 8, "record range");
}

void scoop_metadata_bytes(const ScoopMetadataCheck *check, ScoopByteSpanV1 bytes,
                          const char *field) {
    scoop_metadata_readonly(check, bytes.data, bytes.length, 1, 1, field);
}

ScoopMetadataCheck scoop_record_check(const ScoopImageRegistry *registry,
                                      ScoopRecordKind kind, size_t index) {
    static const char *const names[] = {"storage", "immortal",  "unit",
                                        "type",    "safepoint", "callable"};
    return (ScoopMetadataCheck){registry->loaded, names[kind],
                                registry->tables[kind].entries[index].identity};
}

int scoop_record_address_compare(const void *left, const void *right) {
    uintptr_t a = ((const ScoopRecordAddress *)left)->address;
    uintptr_t b = ((const ScoopRecordAddress *)right)->address;
    return (a > b) - (a < b);
}

size_t scoop_record_address_find(const ScoopRecordAddress *addresses, size_t count,
                                 uintptr_t address) {
    ScoopRecordAddress key = {.address = address};
    const ScoopRecordAddress *found = bsearch(&key, addresses, count, sizeof *addresses,
                                              scoop_record_address_compare);
    return found == NULL ? SIZE_MAX : found->index;
}
