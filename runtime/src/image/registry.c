#include <stdlib.h>
#include <string.h>

#include "internal.h"

ScoopImageRegistry *scoop_image_collect(const ScoopPlatformMetadataImages *loaded,
                                        const ScoopImageDescriptorV1 *const *images,
                                        uint64_t image_count,
                                        const ScoopRootEntryDescriptorV1 *root) {
    ScoopMetadataCheck check = {.loaded = loaded, .kind = "root"};
    if (loaded == NULL || loaded->ranges == NULL || loaded->range_count == 0) {
        scoop_metadata_fatal(&check, "loaded image ranges are absent");
    }
    scoop_metadata_prefix(&check, root, SCOOP_ROOT_ENTRY_DESCRIPTOR_MAGIC_V1,
                          sizeof *root);
    if (scoop_digest_zero(&root->owner_cone_identity) ||
        scoop_digest_zero(&root->callable_id) ||
        scoop_digest_zero(&root->source_signature_fingerprint) ||
        scoop_digest_zero(&root->gateway_callable_id) ||
        scoop_digest_zero(&root->gateway_definition_fingerprint)) {
        scoop_metadata_fatal(&check, "missing identity or fingerprint");
    }
    ScoopImageRegistry *registry = scoop_metadata_allocate(1, sizeof *registry);
    registry->loaded = loaded;
    registry->root = root;
    scoop_image_order(registry, images, image_count);
    scoop_image_records(registry);
    return registry;
}

void scoop_image_registry_dispose(ScoopImageRegistry *registry) {
    if (registry == NULL) {
        return;
    }
    for (size_t kind = 0; kind < SCOOP_RECORD_KIND_COUNT; kind++) {
        free(registry->tables[kind].entries);
        free(registry->tables[kind].addresses);
    }
    free(registry->images);
    free(registry->image_order);
    free(registry->type_addresses);
    free(registry->callable_addresses);
    free(registry->site_ids);
    free(registry->immortal_addresses);
    free(registry->static_roots);
    free(registry->eager_units);
    scoop_metadata_scan_dispose(registry);
    scoop_stackmap_dispose_index(&registry->stackmaps);
    free(registry);
}

bool scoop_records_same_owner(const ScoopRegisteredRecord *left,
                              const ScoopRegisteredRecord *right) {
    if (left->identity->linkage_kind != right->identity->linkage_kind) {
        return false;
    }
    if (left->identity->linkage_kind == SCOOP_REGISTRATION_LINKAGE_STRONG_V1) {
        return left->producer == right->producer;
    }
    return scoop_digest_equal(&left->identity->odr_group_id,
                              &right->identity->odr_group_id);
}
