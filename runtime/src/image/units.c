#include <stdlib.h>

#include "internal.h"

static const ScoopRegisteredRecord *
claim_storage(const ScoopImageRegistry *registry,
              const ScoopMetadataCheck *check,
              const ScoopStaticStorageDescriptorV1 *storage,
              const ScoopRegisteredRecord *owner, bool *claimed, bool failure) {
    const ScoopRegisteredRecord *entry =
        scoop_record_by_address(registry, SCOOP_RECORD_STORAGE, storage);
    if (entry == NULL) {
        scoop_metadata_fatal(check, "unregistered storage");
    }
    size_t index =
        (size_t)(entry - registry->tables[SCOOP_RECORD_STORAGE].entries);
    if (claimed[index] ||
        storage->initial_state_kind !=
            SCOOP_STATIC_INITIAL_ZEROED_FOR_RUNTIME_UNIT_V1 ||
        (owner != NULL && !scoop_records_same_owner(owner, entry))) {
        scoop_metadata_fatal(check,
                             "shared storage role, initial state or producer");
    }
    claimed[index] = true;
    if (failure &&
        (storage->byte_size != 8 || storage->allocation_extent != 8 ||
         storage->required_alignment != 8 ||
         storage->scan_kind != SCOOP_STATIC_SCAN_RECURSIVE_V1 ||
         storage->scan_program[0] != 1 || storage->scan_program[1] != 0)) {
        scoop_metadata_fatal(check,
                             "failure root must be a dedicated reference slot");
    }
    return entry;
}

static const ScoopCallableRegistrationDescriptorV1 *check_callable(
    const ScoopImageRegistry *registry, const ScoopMetadataCheck *check,
    const ScoopRegisteredRecord *owner, const ScoopDigest256V1 *identity,
    ScoopCallableAddressV1 address, const ScoopDigest256V1 *fingerprint) {
    const ScoopRegisteredRecord *entry =
        scoop_record_by_id(registry, SCOOP_RECORD_CALLABLE, identity);
    if (entry == NULL ||
        (owner != NULL && !scoop_records_same_owner(owner, entry))) {
        scoop_metadata_fatal(check, "callable identity or producer");
    }
    const ScoopCallableRegistrationDescriptorV1 *callable = entry->record;
    if (callable->entry != address ||
        (fingerprint != NULL &&
         !scoop_digest_equal(&callable->body_definition_fingerprint,
                             fingerprint))) {
        scoop_metadata_fatal(check, "callable address or fingerprint mirror");
    }
    return callable;
}

static void check_unit(const ScoopImageRegistry *registry, size_t index,
                       bool *claimed) {
    const ScoopRegisteredRecord *entry =
        &registry->tables[SCOOP_RECORD_UNIT].entries[index];
    ScoopMetadataCheck check =
        scoop_record_check(registry, SCOOP_RECORD_UNIT, index);
    const ScoopInitializationUnitDescriptorV1 *unit = entry->record;
    if (unit->reserved_zero != 0 || unit->diagnostic_path.length == 0) {
        scoop_metadata_fatal(&check, "reserved field or empty diagnostic");
    }
    scoop_metadata_bytes(&check, unit->diagnostic_path, "diagnostic path span");
    scoop_metadata_writable(&check, unit->cell, sizeof *unit->cell, 8,
                            "cell writable range");
    if (unit->cell->state != 0 || unit->cell->owner_thread != NULL) {
        scoop_metadata_fatal(&check, "nonzero initial cell");
    }
    claim_storage(registry, &check, unit->storage, entry, claimed, false);
    claim_storage(registry, &check, unit->failure_root, entry, claimed, true);
    check_callable(registry, &check, entry, &unit->initializer_callable_id,
                   unit->initializer_entry, NULL);
    check_callable(registry, &check, entry, &unit->ensure_callable_id,
                   unit->ensure_entry, NULL);
    if (unit->schedule_kind == SCOOP_INITIALIZATION_EAGER_STARTUP_V1) {
        if (entry->identity->linkage_kind !=
            SCOOP_REGISTRATION_LINKAGE_STRONG_V1) {
            scoop_metadata_fatal(&check,
                                 "eager unit must have a defining Cone");
        }
        check_callable(registry, &check, entry,
                       &unit->startup_gateway_callable_id,
                       (ScoopCallableAddressV1)unit->startup_gateway,
                       &unit->startup_gateway_definition_fingerprint);
    } else if (unit->schedule_kind == SCOOP_INITIALIZATION_LAZY_ACCESS_V1) {
        if (unit->startup_gateway != NULL ||
            !scoop_digest_zero(&unit->startup_gateway_callable_id) ||
            !scoop_digest_zero(&unit->startup_gateway_definition_fingerprint)) {
            scoop_metadata_fatal(&check, "lazy unit has a startup gateway");
        }
    } else {
        scoop_metadata_fatal(&check, "unknown schedule kind");
    }
}

static void check_root(const ScoopImageRegistry *registry, bool *claimed) {
    const ScoopRootEntryDescriptorV1 *root = registry->root;
    ScoopMetadataCheck check = {.loaded = registry->loaded, .kind = "root"};
    const ScoopRegisteredRecord *body =
        scoop_record_by_id(registry, SCOOP_RECORD_CALLABLE, &root->callable_id);
    if (body == NULL ||
        body->identity->linkage_kind != SCOOP_REGISTRATION_LINKAGE_STRONG_V1 ||
        !scoop_digest_equal(&body->producer->cone.identity,
                            &root->owner_cone_identity)) {
        scoop_metadata_fatal(&check, "source callable owner");
    }
    check_callable(registry, &check, body, &root->gateway_callable_id,
                   (ScoopCallableAddressV1)root->gateway,
                   &root->gateway_definition_fingerprint);
    claim_storage(registry, &check, root->failure_root, body, claimed, true);
}

void scoop_image_units(ScoopImageRegistry *registry) {
    const ScoopRecordTable *storages = &registry->tables[SCOOP_RECORD_STORAGE];
    const ScoopRecordTable *units = &registry->tables[SCOOP_RECORD_UNIT];
    bool *claimed = scoop_metadata_allocate(storages->count, sizeof *claimed);
    for (size_t index = 0; index < units->count; index++) {
        check_unit(registry, index, claimed);
    }
    check_root(registry, claimed);
    for (size_t index = 0; index < storages->count; index++) {
        const ScoopStaticStorageDescriptorV1 *storage =
            storages->entries[index].record;
        if (storage->initial_state_kind ==
                SCOOP_STATIC_INITIAL_ZEROED_FOR_RUNTIME_UNIT_V1 &&
            !claimed[index]) {
            ScoopMetadataCheck check =
                scoop_record_check(registry, SCOOP_RECORD_STORAGE, index);
            scoop_metadata_fatal(
                &check, "zeroed storage has no initialization or failure role");
        }
    }
    free(claimed);
    registry->eager_units =
        scoop_metadata_allocate(units->count, sizeof *registry->eager_units);
    for (size_t image = 0; image < registry->image_count; image++) {
        for (size_t unit = 0; unit < units->count; unit++) {
            const ScoopInitializationUnitDescriptorV1 *record =
                units->entries[unit].record;
            if (units->entries[unit].producer == registry->image_order[image] &&
                record->schedule_kind ==
                    SCOOP_INITIALIZATION_EAGER_STARTUP_V1) {
                registry->eager_units[registry->eager_unit_count++] = record;
            }
        }
    }
}
