#ifndef SCOOP_TEST_IMAGE_STACKMAP_FIXTURE_H
#define SCOOP_TEST_IMAGE_STACKMAP_FIXTURE_H

#include "../src/gc/stackmap/fingerprint.h"
#include "image_storage_fixture.h"

typedef struct BlobOffsets {
    size_t start, record, locations, live_outs, end;
} BlobOffsets;

typedef struct ScoopStackmapFixture {
    ScoopStorageFixture storage;
    ScoopStackMapImage image;
    size_t length;
    BlobOffsets blobs[3];
} ScoopStackmapFixture;

extern const ScoopManagedFrameOps scoop_darwin_aarch64_managed_frame_ops;
void scoop_test_stackmap_setup(ScoopStackmapFixture *fixture);
void scoop_test_stackmap_rust_vector(void);
void scoop_test_stackmap_fingerprint(ScoopSafepointRegistrationDescriptorV1 *site,
                                     uint32_t instruction_offset);
void scoop_test_stackmap_write(ScoopStackmapFixture *fixture, size_t offset,
                               uint64_t value, size_t size);

#endif
