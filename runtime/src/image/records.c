#include <stdlib.h>
#include <string.h>

#include "internal.h"

typedef struct RecordSpan {
    const void *pointers;
    uint64_t count;
    uint64_t magic;
    size_t size;
    const char *name;
} RecordSpan;

static RecordSpan record_span(const ScoopImageDescriptorV1 *image,
                              ScoopRecordKind kind) {
#define RECORD_SPAN(field, count_field, tag, type, label)                              \
    (RecordSpan) { image->field, image->count_field, tag, sizeof(type), label }
    switch (kind) {
    case SCOOP_RECORD_STORAGE:
        return RECORD_SPAN(static_storages, static_storage_count,
                           SCOOP_STATIC_STORAGE_DESCRIPTOR_MAGIC_V1,
                           ScoopStaticStorageDescriptorV1, "storage");
    case SCOOP_RECORD_IMMORTAL:
        return RECORD_SPAN(immortal_objects, immortal_object_count,
                           SCOOP_IMMORTAL_OBJECT_DESCRIPTOR_MAGIC_V1,
                           ScoopImmortalObjectDescriptorV1, "immortal");
    case SCOOP_RECORD_UNIT:
        return RECORD_SPAN(initialization_units, initialization_unit_count,
                           SCOOP_INITIALIZATION_UNIT_DESCRIPTOR_MAGIC_V1,
                           ScoopInitializationUnitDescriptorV1, "unit");
    case SCOOP_RECORD_TYPE:
        return RECORD_SPAN(type_registrations, type_registration_count,
                           SCOOP_TYPE_REGISTRATION_DESCRIPTOR_MAGIC_V1,
                           ScoopTypeRegistrationDescriptorV1, "type");
    case SCOOP_RECORD_SITE:
        return RECORD_SPAN(safepoints, safepoint_count,
                           SCOOP_SAFEPOINT_REGISTRATION_DESCRIPTOR_MAGIC_V1,
                           ScoopSafepointRegistrationDescriptorV1, "safepoint");
    case SCOOP_RECORD_CALLABLE:
        return RECORD_SPAN(callables, callable_count,
                           SCOOP_CALLABLE_REGISTRATION_DESCRIPTOR_MAGIC_V1,
                           ScoopCallableRegistrationDescriptorV1, "callable");
    case SCOOP_RECORD_KIND_COUNT:
        scoop_metadata_fatal(NULL, "invalid internal record kind");
    }
    scoop_metadata_fatal(NULL, "invalid internal record kind");
#undef RECORD_SPAN
}

static void check_identity(const ScoopMetadataCheck *check) {
    const ScoopRegistrationIdentityV1 *identity = check->identity;
    if (identity->reserved_zero != 0 || scoop_digest_zero(&identity->semantic_id) ||
        scoop_digest_zero(&identity->definition_fingerprint)) {
        scoop_metadata_fatal(check, "reserved field or empty identity/fingerprint");
    }
    bool group_zero = scoop_digest_zero(&identity->odr_group_id);
    bool member_zero = scoop_digest_zero(&identity->odr_member_id);
    if ((identity->linkage_kind == SCOOP_REGISTRATION_LINKAGE_STRONG_V1 && group_zero &&
         member_zero) ||
        (identity->linkage_kind == SCOOP_REGISTRATION_LINKAGE_ODR_V1 && !group_zero &&
         !member_zero)) {
        return;
    }
    scoop_metadata_fatal(check, "linkage or ODR group/member fields");
}

static int compare_records(const void *left, const void *right) {
    const ScoopRegisteredRecord *a = left, *b = right;
    return memcmp(a->identity->semantic_id.bytes, b->identity->semantic_id.bytes, 32);
}

static void collect_table(ScoopImageRegistry *registry, ScoopRecordKind kind) {
    ScoopRecordTable *table = &registry->tables[kind];
    size_t count = 0;
    for (size_t index = 0; index < registry->image_count; index++) {
        RecordSpan span = record_span(registry->images[index], kind);
        ScoopMetadataCheck check = {.loaded = registry->loaded, .kind = span.name};
        scoop_metadata_readonly(&check, span.pointers, span.count, sizeof(void *), 8,
                                "producer pointer span");
        if (span.count > SIZE_MAX - count) {
            scoop_metadata_fatal(&check, "producer count overflow");
        }
        count += (size_t)span.count;
    }
    table->entries = scoop_metadata_allocate(count, sizeof *table->entries);
    for (size_t image = 0; image < registry->image_count; image++) {
        RecordSpan span = record_span(registry->images[image], kind);
        for (size_t index = 0; index < span.count; index++) {
            const void *record;
            memcpy(&record, (const uint8_t *)span.pointers + index * sizeof record,
                   sizeof record);
            ScoopMetadataCheck check = {.loaded = registry->loaded, .kind = span.name};
            scoop_metadata_prefix(&check, record, span.magic, span.size);
            check.identity =
                (const ScoopRegistrationIdentityV1 *)((const uint8_t *)record +
                                                      sizeof(ScoopDescriptorPrefixV1));
            check_identity(&check);
            table->entries[table->count++] = (ScoopRegisteredRecord){
                record, check.identity, registry->images[image]};
        }
    }
    qsort(table->entries, table->count, sizeof *table->entries, compare_records);
    size_t unique = 0;
    for (size_t index = 0; index < table->count; index++) {
        ScoopRegisteredRecord *entry = &table->entries[index];
        if (unique != 0 && compare_records(entry, &table->entries[unique - 1]) == 0) {
            ScoopMetadataCheck check = scoop_record_check(registry, kind, index);
            if (entry->identity->linkage_kind != SCOOP_REGISTRATION_LINKAGE_ODR_V1 ||
                table->entries[unique - 1].identity->linkage_kind !=
                    SCOOP_REGISTRATION_LINKAGE_ODR_V1) {
                scoop_metadata_fatal(&check, "duplicate Strong producer");
            }
            if (entry->record != table->entries[unique - 1].record) {
                scoop_metadata_fatal(&check, "ODR record address was not coalesced");
            }
            continue;
        }
        table->entries[unique++] = *entry;
    }
    table->count = unique;
    table->addresses = scoop_metadata_allocate(unique, sizeof *table->addresses);
    for (size_t index = 0; index < unique; index++) {
        table->addresses[index] =
            (ScoopRecordAddress){(uintptr_t)table->entries[index].record, index};
    }
    qsort(table->addresses, unique, sizeof *table->addresses,
          scoop_record_address_compare);
}

typedef struct OdrMember {
    ScoopRecordKind kind;
    const ScoopRegisteredRecord *entry;
} OdrMember;

static int compare_members(const void *left, const void *right) {
    const ScoopRegistrationIdentityV1 *a = ((const OdrMember *)left)->entry->identity;
    const ScoopRegistrationIdentityV1 *b = ((const OdrMember *)right)->entry->identity;
    int group = memcmp(a->odr_group_id.bytes, b->odr_group_id.bytes, 32);
    return group ? group : memcmp(a->odr_member_id.bytes, b->odr_member_id.bytes, 32);
}

static void check_members(const ScoopImageRegistry *registry) {
    size_t count = 0;
    for (size_t kind = 0; kind < SCOOP_RECORD_KIND_COUNT; kind++) {
        if (registry->tables[kind].count > SIZE_MAX - count) {
            scoop_metadata_fatal(NULL, "ODR member count overflow");
        }
        count += registry->tables[kind].count;
    }
    OdrMember *members = scoop_metadata_allocate(count, sizeof *members);
    count = 0;
    for (size_t kind = 0; kind < SCOOP_RECORD_KIND_COUNT; kind++) {
        const ScoopRecordTable *table = &registry->tables[kind];
        for (size_t index = 0; index < table->count; index++) {
            if (table->entries[index].identity->linkage_kind ==
                SCOOP_REGISTRATION_LINKAGE_ODR_V1) {
                members[count++] =
                    (OdrMember){(ScoopRecordKind)kind, &table->entries[index]};
            }
        }
    }
    qsort(members, count, sizeof *members, compare_members);
    for (size_t index = 1; index < count; index++) {
        if (compare_members(&members[index - 1], &members[index]) == 0) {
            ScoopMetadataCheck check = {.loaded = registry->loaded,
                                        .kind = "ODR member",
                                        .identity = members[index].entry->identity};
            scoop_metadata_fatal(&check,
                                 "group/member names different records or kinds");
        }
    }
    free(members);
}

void scoop_image_records(ScoopImageRegistry *registry) {
    for (size_t kind = 0; kind < SCOOP_RECORD_KIND_COUNT; kind++) {
        collect_table(registry, (ScoopRecordKind)kind);
    }
    check_members(registry);
}
