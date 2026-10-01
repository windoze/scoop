#ifndef SCOOP_IMAGE_INTERNAL_H
#define SCOOP_IMAGE_INTERNAL_H

#include "registry.h"

typedef struct ScoopMetadataCheck {
    const ScoopPlatformMetadataImages *loaded;
    const char *kind;
    const ScoopRegistrationIdentityV1 *identity;
} ScoopMetadataCheck;

_Noreturn void scoop_metadata_fatal(const ScoopMetadataCheck *check, const char *field);
void *scoop_metadata_allocate(size_t count, size_t size);
bool scoop_digest_zero(const ScoopDigest256V1 *digest);
bool scoop_digest_equal(const ScoopDigest256V1 *left, const ScoopDigest256V1 *right);
void scoop_metadata_readonly(const ScoopMetadataCheck *check, const void *pointer,
                             uint64_t count, size_t size, size_t alignment,
                             const char *field);
void scoop_metadata_writable(const ScoopMetadataCheck *check, void *pointer,
                             uint64_t size, uint64_t alignment, const char *field);
void scoop_metadata_prefix(const ScoopMetadataCheck *check, const void *record,
                           uint64_t magic, size_t size);
void scoop_metadata_bytes(const ScoopMetadataCheck *check, ScoopByteSpanV1 bytes,
                          const char *field);
void scoop_metadata_executable(const ScoopMetadataCheck *check,
                               ScoopCallableAddressV1 entry, const char *field);
ScoopMetadataCheck scoop_record_check(const ScoopImageRegistry *registry,
                                      ScoopRecordKind kind, size_t index);
int scoop_record_address_compare(const void *left, const void *right);
size_t scoop_record_address_find(const ScoopRecordAddress *addresses, size_t count,
                                 uintptr_t address);
const ScoopRegisteredRecord *scoop_record_by_id(const ScoopImageRegistry *registry,
                                                ScoopRecordKind kind,
                                                const ScoopDigest256V1 *identity);
const ScoopRegisteredRecord *scoop_record_by_address(const ScoopImageRegistry *registry,
                                                     ScoopRecordKind kind,
                                                     const void *record);
void scoop_image_order(ScoopImageRegistry *registry,
                       const ScoopImageDescriptorV1 *const *images,
                       uint64_t image_count);
void scoop_image_records(ScoopImageRegistry *registry);
bool scoop_records_same_owner(const ScoopRegisteredRecord *left,
                              const ScoopRegisteredRecord *right);
void scoop_metadata_scan(ScoopImageRegistry *registry, const ScoopMetadataCheck *check,
                         const uint64_t *scan);
void scoop_metadata_scan_dispose(ScoopImageRegistry *registry);
void scoop_image_type_relations(const ScoopImageRegistry *registry);

#endif
