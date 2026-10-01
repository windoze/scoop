#ifndef SCOOP_STACKMAP_FINGERPRINT_H
#define SCOOP_STACKMAP_FINGERPRINT_H

#include "../stackmap.h"
#include "scoop_runtime_metadata_v1.h"

bool scoop_stackmap_fingerprint(const ScoopStackMapRecord *record,
                                const ScoopSafepointRegistrationDescriptorV1 *site,
                                ScoopDigest256V1 *fingerprint);
bool scoop_stackmap_same_payload(const ScoopStackMapRecord *left,
                                 const ScoopStackMapRecord *right);

#endif
