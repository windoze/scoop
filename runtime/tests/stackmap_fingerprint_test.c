#include <assert.h>
#include <stdio.h>
#include <string.h>

#include "image_stackmap_fixture.h"

static void digest(ScoopDigest256V1 *output, const char *text) {
    for (size_t index = 0; index < 32; index++) {
        unsigned value;
        assert(sscanf(text + index * 2, "%2x", &value) == 1);
        output->bytes[index] = (uint8_t)value;
    }
}

/* This vector is also asserted by the Rust artifact normalizer. */
void scoop_test_stackmap_rust_vector(void) {
    ScoopSafepointRegistrationDescriptorV1 site = {.safepoint_id =
                                                       UINT64_C(0xbb222eea00c1897e),
                                                   .site_role = 1,
                                                   .root_pair_count = 1};
    digest(&site.registration.semantic_id,
           "b5982017f75e012110a9e3ffda3958a9cc516e2426741f0ebf5160808b38c0d4");
    digest(&site.owner_callable_id,
           "ec077aa72907e24e6afebd82d42c3e42c379acf1bc526aeea1302c04bcc634fd");
    ScoopStackMapRootPair pair = {.base = {.kind = SCOOP_STACKMAP_INDIRECT,
                                           .size = 8,
                                           .dwarf_register = 31,
                                           .offset = 16},
                                  .derived = {.kind = SCOOP_STACKMAP_INDIRECT,
                                              .size = 8,
                                              .dwarf_register = 31,
                                              .offset = 16}};
    ScoopStackMapRecord record = {.safepoint_id = site.safepoint_id,
                                  .instruction_offset = 8,
                                  .stack_size = 64,
                                  .roots = &pair,
                                  .root_count = 1};
    for (size_t index = 0; index < 3; index++) {
        record.header[index] =
            (ScoopStackMapLocation){.kind = SCOOP_STACKMAP_CONSTANT, .size = 8};
    }
    ScoopDigest256V1 expected, actual;
    digest(&expected,
           "94ca32b8033bbdef539ed5e240770aa1d6c68f2bb6af88603efd35279e47a00f");
    assert(scoop_stackmap_fingerprint(&record, &site, &actual));
    assert(scoop_digest_equal(&expected, &actual));
    record.header[0].kind = SCOOP_STACKMAP_CONSTANT_INDEX;
    record.header[0].offset = 999; /* The parser already resolved this index to zero. */
    assert(scoop_stackmap_fingerprint(&record, &site, &actual));
    assert(scoop_digest_equal(&expected, &actual));
}
