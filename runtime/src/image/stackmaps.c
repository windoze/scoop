#include <inttypes.h>
#include <stdio.h>
#include <stdlib.h>

#include "internal.h"

static _Noreturn void parse_fatal(ScoopStackMapError error) {
    fprintf(stderr, "scoop runtime stackmap: image %zu offset %zu: %s\n",
            error.image_index, error.section_offset,
            scoop_stackmap_error_message(error.code));
    abort();
}

static _Noreturn void record_fatal(const ScoopStackMapRecord *record,
                                   const char *field) {
    fprintf(stderr,
            "scoop runtime stackmap: image %zu offset %zu site %" PRIu64 ": %s\n",
            record->image_index, record->section_offset, record->safepoint_id, field);
    abort();
}

static void validate_site(const ScoopImageRegistry *registry, const ScoopManagedFrameOps *frames,
                          const ScoopStackMapRecord *record, size_t index) {
    const ScoopRecordTable *sites = &registry->tables[SCOOP_RECORD_SITE];
    const ScoopSafepointRegistrationDescriptorV1 *site = sites->entries[index].record;
    const ScoopRegisteredRecord *owner =
        scoop_record_by_id(registry, SCOOP_RECORD_CALLABLE, &site->owner_callable_id);
    if (owner == NULL || !scoop_records_same_owner(&sites->entries[index], owner)) {
        ScoopMetadataCheck check = scoop_record_check(registry, SCOOP_RECORD_SITE, index);
        scoop_metadata_fatal(&check, "owner callable identity or producer");
    }
    const ScoopCallableRegistrationDescriptorV1 *callable = owner->record;
    if ((uintptr_t)callable->entry != record->function_address ||
        record->root_count != site->root_pair_count) {
        record_fatal(record, "owner entry address or root count");
    }
    ScoopPlatformError platform_error = {0};
    if (!frames->validate_record(record, &platform_error)) {
        fprintf(stderr, "scoop runtime stackmap: root %u platform error %u\n",
                (unsigned)platform_error.root_index, (unsigned)platform_error.code);
        record_fatal(record, "invalid managed frame or root location");
    }
}

void scoop_image_stackmaps(ScoopImageRegistry *registry,
                           const ScoopManagedFrameOps *frames) {
    if (registry->site_ids == NULL || registry->stackmaps.records != NULL ||
        frames == NULL || frames->validate_record == NULL ||
        (registry->loaded->count != 0 && registry->loaded->images == NULL)) {
        scoop_metadata_fatal(NULL, "stackmap registration order or platform");
    }
    ScoopMetadataCheck check = {.loaded = registry->loaded, .kind = "stackmap"};
    for (size_t index = 0; index < registry->loaded->count; index++) {
        const ScoopStackMapImage *image = &registry->loaded->images[index];
        scoop_metadata_range(&check, image->section, image->section_size, 1, 8,
                             "complete section range");
    }
    ScoopStackMapIndex raw;
    ScoopStackMapError error;
    if (!scoop_stackmap_read_records(registry->loaded->images, registry->loaded->count,
                                     &raw, &error)) {
        parse_fatal(error);
    }
    const ScoopRecordTable *sites = &registry->tables[SCOOP_RECORD_SITE];
    ScoopStackMapIndex joined = {
        .records = scoop_metadata_allocate(sites->count, sizeof *joined.records),
        .record_count = sites->count};
    bool *seen = scoop_metadata_allocate(sites->count, sizeof *seen);
    for (size_t index = 0; index < raw.record_count; index++) {
        ScoopStackMapRecord *record = &raw.records[index];
        size_t site = scoop_record_address_find(registry->site_ids, sites->count,
                                                (uintptr_t)record->safepoint_id);
        if (site == SIZE_MAX) {
            record_fatal(record, "raw record has no safepoint registration");
        }
        if (!seen[site]) {
            validate_site(registry, frames, record, site);
            seen[site] = true;
            joined.records[site] = *record;
            *record = (ScoopStackMapRecord){0};
            continue;
        }
        const ScoopStackMapRecord *previous = &joined.records[site];
        if (sites->entries[site].identity->linkage_kind !=
            SCOOP_REGISTRATION_LINKAGE_ODR_V1) {
            record_fatal(record, "duplicate Strong raw record");
        }
        if (previous->return_pc != record->return_pc ||
            previous->function_address != record->function_address ||
            !scoop_stackmap_same_payload(previous, record)) {
            record_fatal(record, "ODR raw records differ in PC or canonical payload");
        }
    }
    scoop_stackmap_dispose_index(&raw);
    for (size_t site = 0; site < sites->count; site++) {
        if (!seen[site]) {
            ScoopMetadataCheck site_check =
                scoop_record_check(registry, SCOOP_RECORD_SITE, site);
            scoop_metadata_fatal(&site_check,
                                 "safepoint registration has no raw record");
        }
    }
    free(seen);
    if (!scoop_stackmap_finish_index(&joined, &error)) {
        parse_fatal(error);
    }
    registry->stackmaps = joined;
}
