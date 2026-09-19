#ifndef SCOOP_VALUE_SHAPE_H
#define SCOOP_VALUE_SHAPE_H

#include "scoop_rt.h"

/* Shared by checked representation operations and the collector. These are
 * the qualified target's managed-storage limits, not source integer types. */
#define SCOOP_MAXIMUM_MANAGED_ALIGNMENT UINT64_C(16)
#define SCOOP_MAXIMUM_MANAGED_OBJECT_SIZE ((uint64_t)INT64_MAX)

_Noreturn void scoop_shape_fatal(const char *message);
uint64_t scoop_shape_add(uint64_t left, uint64_t right);
uint64_t scoop_shape_align(uint64_t size, uint64_t alignment);
const ScoopTypeInstanceShapeV1 *scoop_shape_require(const ScoopTypeDescriptor *td,
                                                    uint32_t kind);
void scoop_shape_validate(const ScoopTypeDescriptor *td);
size_t scoop_shape_allocation_size(const ScoopTypeDescriptor *td, uint64_t count);
size_t scoop_shape_normalize_allocation(const ScoopTypeDescriptor *td,
                                        size_t requested);
void scoop_shape_validate_object(const void *object, size_t allocation_size);
bool scoop_shape_scan_equal(const uint64_t *left, const uint64_t *right,
                            uint64_t translation);
void scoop_shape_scan_validate(const uint64_t *scan, uint64_t extent);

#endif
