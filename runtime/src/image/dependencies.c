#include <stdlib.h>
#include <string.h>

#include "internal.h"

static int compare_images(const void *left, const void *right) {
    const ScoopImageDescriptorV1 *a = *(const ScoopImageDescriptorV1 *const *)left;
    const ScoopImageDescriptorV1 *b = *(const ScoopImageDescriptorV1 *const *)right;
    return memcmp(a->cone.identity.bytes, b->cone.identity.bytes, 32);
}

static size_t find_image(const ScoopImageRegistry *registry,
                         const ScoopDigest256V1 *identity) {
    size_t low = 0, high = registry->image_count;
    while (low < high) {
        size_t mid = low + (high - low) / 2;
        int comparison =
            memcmp(identity->bytes, registry->images[mid]->cone.identity.bytes, 32);
        if (comparison == 0) {
            return mid;
        }
        if (comparison < 0) {
            high = mid;
        } else {
            low = mid + 1;
        }
    }
    scoop_metadata_fatal(NULL, "image dependency or root owner is absent");
}

static int compare_bytes(ScoopByteSpanV1 left, ScoopByteSpanV1 right) {
    uint64_t length = left.length < right.length ? left.length : right.length;
    int order = memcmp(left.data, right.data, (size_t)length);
    return order ? order : (left.length > right.length) - (left.length < right.length);
}

static int compare_coordinates(const ScoopConeRecordV1 *left,
                               const ScoopConeRecordV1 *right) {
    int order = compare_bytes(left->group, right->group);
    if (order == 0) {
        order = compare_bytes(left->name, right->name);
    }
    /* A group/name can occur only once in the input closure, so the version
     * tie breaker never chooses between two versions of the same Cone. */
    return order ? order : compare_bytes(left->version, right->version);
}

typedef struct ImageEdges {
    size_t *dependencies;
    size_t count;
    bool reachable;
    bool processed;
} ImageEdges;

static void collect_images(ScoopImageRegistry *registry,
                           const ScoopImageDescriptorV1 *const *images,
                           uint64_t image_count) {
    ScoopMetadataCheck check = {.loaded = registry->loaded, .kind = "image"};
    if (image_count == 0) {
        scoop_metadata_fatal(&check, "empty input closure");
    }
    scoop_metadata_readonly(&check, images, image_count, sizeof *images, 8,
                            "input pointer span");
    registry->image_count = (size_t)image_count;
    registry->images = scoop_metadata_allocate((size_t)image_count, sizeof *images);
    for (size_t index = 0; index < image_count; index++) {
        const ScoopImageDescriptorV1 *image = images[index];
        scoop_metadata_prefix(&check, image, SCOOP_IMAGE_DESCRIPTOR_MAGIC_V1,
                              sizeof *image);
        scoop_metadata_bytes(&check, image->cone.group, "group span");
        scoop_metadata_bytes(&check, image->cone.name, "name span");
        scoop_metadata_bytes(&check, image->cone.version, "version span");
        if (image->cone.name.length == 0 || image->cone.version.length == 0 ||
            scoop_digest_zero(&image->cone.identity) ||
            scoop_digest_zero(&image->runtime_image_fingerprint)) {
            scoop_metadata_fatal(&check, "empty coordinate, identity or fingerprint");
        }
        scoop_metadata_readonly(&check, image->dependencies, image->dependency_count,
                                sizeof *image->dependencies, _Alignof(ScoopDigest256V1),
                                "dependency span");
        registry->images[index] = image;
    }
    qsort(registry->images, registry->image_count, sizeof *registry->images,
          compare_images);
    for (size_t index = 0; index < image_count; index++) {
        const ScoopConeRecordV1 *cone = &registry->images[index]->cone;
        if (index != 0 &&
            scoop_digest_equal(&cone->identity,
                               &registry->images[index - 1]->cone.identity)) {
            scoop_metadata_fatal(&check, "duplicate identity");
        }
        for (size_t previous = 0; previous < index; previous++) {
            const ScoopConeRecordV1 *other = &registry->images[previous]->cone;
            if (compare_bytes(cone->group, other->group) == 0 &&
                compare_bytes(cone->name, other->name) == 0) {
                scoop_metadata_fatal(&check,
                                     "duplicate group/name or multiple versions");
            }
        }
    }
}

static ImageEdges *collect_edges(const ScoopImageRegistry *registry) {
    ImageEdges *edges = scoop_metadata_allocate(registry->image_count, sizeof *edges);
    for (size_t index = 0; index < registry->image_count; index++) {
        const ScoopImageDescriptorV1 *image = registry->images[index];
        edges[index].count = (size_t)image->dependency_count;
        edges[index].dependencies =
            scoop_metadata_allocate(edges[index].count, sizeof(size_t));
        for (size_t dep = 0; dep < edges[index].count; dep++) {
            size_t target = find_image(registry, &image->dependencies[dep]);
            if (target == index) {
                scoop_metadata_fatal(NULL, "image self dependency");
            }
            for (size_t previous = 0; previous < dep; previous++) {
                if (edges[index].dependencies[previous] == target) {
                    scoop_metadata_fatal(NULL, "duplicate image dependency");
                }
            }
            edges[index].dependencies[dep] = target;
        }
    }
    return edges;
}

static void check_reachability(const ScoopImageRegistry *registry, ImageEdges *edges) {
    size_t *pending = scoop_metadata_allocate(registry->image_count, sizeof *pending);
    size_t count = 0;
    size_t root = find_image(registry, &registry->root->owner_cone_identity);
    edges[root].reachable = true;
    pending[count++] = root;
    while (count != 0) {
        const ImageEdges *current = &edges[pending[--count]];
        for (size_t dep = 0; dep < current->count; dep++) {
            size_t target = current->dependencies[dep];
            if (!edges[target].reachable) {
                edges[target].reachable = true;
                pending[count++] = target;
            }
        }
    }
    free(pending);
    for (size_t index = 0; index < registry->image_count; index++) {
        if (!edges[index].reachable) {
            scoop_metadata_fatal(NULL, "image outside the root dependency closure");
        }
    }
}

void scoop_image_order(ScoopImageRegistry *registry,
                       const ScoopImageDescriptorV1 *const *images,
                       uint64_t image_count) {
    collect_images(registry, images, image_count);
    ImageEdges *edges = collect_edges(registry);
    check_reachability(registry, edges);
    registry->image_order =
        scoop_metadata_allocate(registry->image_count, sizeof *registry->image_order);
    for (size_t output = 0; output < registry->image_count; output++) {
        size_t next = SIZE_MAX;
        for (size_t index = 0; index < registry->image_count; index++) {
            bool ready = !edges[index].processed;
            for (size_t dep = 0; ready && dep < edges[index].count; dep++) {
                ready = edges[edges[index].dependencies[dep]].processed;
            }
            if (ready && (next == SIZE_MAX ||
                          compare_coordinates(&registry->images[index]->cone,
                                              &registry->images[next]->cone) < 0)) {
                next = index;
            }
        }
        if (next == SIZE_MAX) {
            scoop_metadata_fatal(NULL, "cyclic image dependencies");
        }
        edges[next].processed = true;
        registry->image_order[output] = registry->images[next];
    }
    for (size_t index = 0; index < registry->image_count; index++) {
        free(edges[index].dependencies);
    }
    free(edges);
}
