#ifndef SCOOP_TEST_IMAGE_FIXTURE_H
#define SCOOP_TEST_IMAGE_FIXTURE_H

#include "../../src/image/registry.h"

ScoopStackMapIndex scoop_test_stackmaps(void);

/* Focused GC tests supply the resolved ABI records directly. Loaded-image
 * validation and actual compiler output are exercised by separate tests. */
void scoop_test_image_init(const ScoopTypeDescriptor *const *types, size_t type_count,
                           const ScoopStaticStorageDescriptorV1 *const *roots,
                           size_t root_count,
                           const ScoopImmortalObjectDescriptorV1 *const *immortals,
                           size_t immortal_count);

#endif
