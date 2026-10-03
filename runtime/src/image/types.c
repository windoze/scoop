#include <stdlib.h>
#include <string.h>

#include "../value_shape.h"
#include "internal.h"

static void unique_addresses(const ScoopImageRegistry *registry, ScoopRecordKind kind,
                             ScoopRecordAddress *addresses, const char *field) {
    size_t count = registry->tables[kind].count;
    qsort(addresses, count, sizeof *addresses, scoop_record_address_compare);
    for (size_t index = 1; index < count; index++) {
        if (addresses[index - 1].address == addresses[index].address) {
            ScoopMetadataCheck check =
                scoop_record_check(registry, kind, addresses[index].index);
            scoop_metadata_fatal(&check, field);
        }
    }
}

static void collect_types(ScoopImageRegistry *registry) {
    const ScoopRecordTable *table = &registry->tables[SCOOP_RECORD_TYPE];
    registry->type_addresses =
        scoop_metadata_allocate(table->count, sizeof *registry->type_addresses);
    ScoopRecordAddress *runtime_ids =
        scoop_metadata_allocate(table->count, sizeof *runtime_ids);
    for (size_t index = 0; index < table->count; index++) {
        ScoopMetadataCheck check =
            scoop_record_check(registry, SCOOP_RECORD_TYPE, index);
        const ScoopTypeRegistrationDescriptorV1 *type = table->entries[index].record;
        if (type->reserved_zero != 0 || type->runtime_type_id == 0 ||
            scoop_digest_zero(&type->descriptor_fingerprint) ||
            scoop_digest_zero(&type->layout_fingerprint)) {
            scoop_metadata_fatal(&check, "reserved field, runtime ID or fingerprint");
        }
        const ScoopTypeDescriptor *td = type->descriptor;
        scoop_metadata_readonly(&check, td, 1, sizeof *td, 8,
                                "TypeDescriptor header range");
        scoop_metadata_readonly(&check, td,
                                sizeof *td + (uint64_t)td->related_type_count * 8, 1, 8,
                                "TypeDescriptor related types range");
        if (td->type_id != type->runtime_type_id) {
            scoop_metadata_fatal(&check, "TypeDescriptor runtime ID mirror");
        }
        scoop_metadata_bytes(&check, td->diagnostic_name, "diagnostic name span");
        if (td->diagnostic_name.length == 0) {
            scoop_metadata_fatal(&check, "empty diagnostic name");
        }
        registry->type_addresses[index] = (ScoopRecordAddress){(uintptr_t)td, index};
        runtime_ids[index] =
            (ScoopRecordAddress){(uintptr_t)type->runtime_type_id, index};
    }
    unique_addresses(registry, SCOOP_RECORD_TYPE, registry->type_addresses,
                     "different exact types share a TypeDescriptor address");
    unique_addresses(registry, SCOOP_RECORD_TYPE, runtime_ids,
                     "64-bit runtime type ID collision");
    free(runtime_ids);
}

static void collect_callables(ScoopImageRegistry *registry) {
    const ScoopRecordTable *table = &registry->tables[SCOOP_RECORD_CALLABLE];
    registry->callable_addresses =
        scoop_metadata_allocate(table->count, sizeof *registry->callable_addresses);
    for (size_t index = 0; index < table->count; index++) {
        ScoopMetadataCheck check =
            scoop_record_check(registry, SCOOP_RECORD_CALLABLE, index);
        const ScoopCallableRegistrationDescriptorV1 *callable =
            table->entries[index].record;
        if (scoop_digest_zero(&callable->body_definition_fingerprint)) {
            scoop_metadata_fatal(&check, "empty body fingerprint");
        }
        scoop_metadata_executable(&check, callable->entry, "entry executable range");
        registry->callable_addresses[index] =
            (ScoopRecordAddress){(uintptr_t)callable->entry, index};
    }
    unique_addresses(registry, SCOOP_RECORD_CALLABLE, registry->callable_addresses,
                     "different callable bodies share an entry address");
}

static void collect_sites(ScoopImageRegistry *registry) {
    const ScoopRecordTable *table = &registry->tables[SCOOP_RECORD_SITE];
    registry->site_ids =
        scoop_metadata_allocate(table->count, sizeof *registry->site_ids);
    for (size_t index = 0; index < table->count; index++) {
        ScoopMetadataCheck check =
            scoop_record_check(registry, SCOOP_RECORD_SITE, index);
        const ScoopSafepointRegistrationDescriptorV1 *site =
            table->entries[index].record;
        if (site->safepoint_id == 0 ||
            site->site_role < SCOOP_SAFEPOINT_MANAGED_POLL_V1 ||
            site->site_role > SCOOP_SAFEPOINT_NATIVE_BORROWED_TRANSITION_V1 ||
            site->root_pair_count > (UINT16_MAX - 3) / 2 ||
            scoop_digest_zero(&site->normalized_stackmap_fingerprint)) {
            scoop_metadata_fatal(&check, "site ID, role, root count or fingerprint");
        }
        const ScoopRegisteredRecord *owner = scoop_record_by_id(
            registry, SCOOP_RECORD_CALLABLE, &site->owner_callable_id);
        if (owner == NULL || !scoop_records_same_owner(&table->entries[index], owner)) {
            scoop_metadata_fatal(&check, "owner callable identity or producer");
        }
        registry->site_ids[index] =
            (ScoopRecordAddress){(uintptr_t)site->safepoint_id, index};
    }
    unique_addresses(registry, SCOOP_RECORD_SITE, registry->site_ids,
                     "64-bit safepoint ID collision");
}

static void check_release_hook(const ScoopImageRegistry *registry,
                               const ScoopMetadataCheck *check,
                               const ScoopTypeRegistrationDescriptorV1 *type) {
    const ScoopTypeDescriptor *td = type->descriptor;
    if (td->release_hook == NULL) {
        return;
    }
    if (td->instance_shape.instance_kind != SCOOP_TYPE_INSTANCE_FIXED_OBJECT_V1) {
        scoop_metadata_fatal(check, "release hook requires a fixed object");
    }
    /* RuntimeEncode: ByteSpan(domain), u32 tag 5, exact type ID. */
    static const char domain[] = "scoop-callable-body-v2";
    uint8_t encoding[8 + sizeof domain - 1 + 4 + 32] = {0};
    encoding[0] = sizeof domain - 1;
    memcpy(encoding + 8, domain, sizeof domain - 1);
    encoding[8 + sizeof domain - 1] = 5;
    memcpy(encoding + 8 + sizeof domain - 1 + 4,
           type->registration.semantic_id.bytes, 32);
    ScoopDigest256V1 body_id;
    if (!scoop_platform_sha256(encoding, sizeof encoding, body_id.bytes)) {
        scoop_metadata_fatal(check, "release hook identity digest");
    }
    const ScoopRegisteredRecord *record =
        scoop_record_by_id(registry, SCOOP_RECORD_CALLABLE, &body_id);
    if (record == NULL) {
        scoop_metadata_fatal(check, "release hook callable registration");
    }
    const ScoopCallableRegistrationDescriptorV1 *callable = record->record;
    if ((uintptr_t)callable->entry != (uintptr_t)td->release_hook) {
        scoop_metadata_fatal(check, "release hook entry disagrees with exact owner");
    }
}

void scoop_image_validate_code_and_types(ScoopImageRegistry *registry) {
    if (registry->type_addresses != NULL) {
        scoop_metadata_fatal(NULL, "type registration repeated");
    }
    collect_types(registry);
    collect_callables(registry);
    collect_sites(registry);
    scoop_image_type_relations(registry);
    const ScoopRecordTable *types = &registry->tables[SCOOP_RECORD_TYPE];
    for (size_t index = 0; index < types->count; index++) {
        ScoopMetadataCheck check =
            scoop_record_check(registry, SCOOP_RECORD_TYPE, index);
        const ScoopTypeRegistrationDescriptorV1 *type = types->entries[index].record;
        const ScoopTypeDescriptor *td = type->descriptor;
        scoop_metadata_scan(registry, &check, td->object_scan);
        scoop_metadata_scan(registry, &check, td->instance_shape.inline_scan);
        scoop_shape_validate(td);
        check_release_hook(registry, &check, type);
    }
}
