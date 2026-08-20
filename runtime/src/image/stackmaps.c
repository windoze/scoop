#include <inttypes.h>
#include <stdio.h>
#include <stdlib.h>

#include "../gc/stackmap/fingerprint.h"
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

static size_t resolve_site(const ScoopImageRegistry *registry,
                           const ScoopManagedFrameOps *frames,
                           const ScoopStackMapRecord *record) {
    const ScoopRecordTable *sites = &registry->tables[SCOOP_RECORD_SITE];
    size_t index = scoop_record_address_find(registry->site_ids, sites->count,
                                             (uintptr_t)record->safepoint_id);
    if (index == SIZE_MAX) {
        record_fatal(record, "raw record has no safepoint registration");
    }
    const ScoopSafepointRegistrationDescriptorV1 *site = sites->entries[index].record;
    const ScoopRegisteredRecord *owner =
        scoop_record_by_id(registry, SCOOP_RECORD_CALLABLE, &site->owner_callable_id);
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
    ScoopDigest256V1 fingerprint;
    if (!scoop_stackmap_fingerprint(record, site, &fingerprint)) {
        record_fatal(record, "cannot normalize or hash record");
    }
    if (!scoop_digest_equal(&fingerprint, &site->normalized_stackmap_fingerprint)) {
        record_fatal(record, "normalized fingerprint mismatch");
    }
    return index;
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
        scoop_metadata_readonly(&check, image->section, image->section_size, 1, 8,
                                "complete section readonly range");
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
        size_t site = resolve_site(registry, frames, record);
        if (!seen[site]) {
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
