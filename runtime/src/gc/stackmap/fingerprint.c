#include <stdlib.h>
#include <string.h>

#include "../../platform/platform.h"
#include "fingerprint.h"

typedef struct StackmapEncoding {
    uint8_t *bytes;
    size_t length;
} StackmapEncoding;

static void encode_u32(StackmapEncoding *encoding, uint32_t value) {
    for (size_t byte = 0; byte < 4; byte++) {
        encoding->bytes[encoding->length++] = (uint8_t)(value >> (byte * 8));
    }
}

static void encode_u64(StackmapEncoding *encoding, uint64_t value) {
    for (size_t byte = 0; byte < 8; byte++) {
        encoding->bytes[encoding->length++] = (uint8_t)(value >> (byte * 8));
    }
}

static void encode_bytes(StackmapEncoding *encoding, const void *bytes, size_t size) {
    memcpy(encoding->bytes + encoding->length, bytes, size);
    encoding->length += size;
}

static bool encode_location(StackmapEncoding *encoding,
                            const ScoopStackMapLocation *location) {
    uint32_t tag = location->kind == SCOOP_STACKMAP_CONSTANT_INDEX
                       ? SCOOP_STACKMAP_CONSTANT
                       : (uint32_t)location->kind;
    encode_u32(encoding, tag);
    encode_u32(encoding, location->size);
    switch (tag) {
    case SCOOP_STACKMAP_REGISTER:
        encode_u32(encoding, location->dwarf_register);
        return location->offset == 0;
    case SCOOP_STACKMAP_DIRECT:
    case SCOOP_STACKMAP_INDIRECT:
        encode_u32(encoding, location->dwarf_register);
        encode_u64(encoding, (uint64_t)(int64_t)location->offset);
        return true;
    case SCOOP_STACKMAP_CONSTANT:
        encode_u64(encoding, location->constant);
        return location->dwarf_register == 0;
    default:
        return false;
    }
}

bool scoop_stackmap_fingerprint(const ScoopStackMapRecord *record,
                                const ScoopSafepointRegistrationDescriptorV1 *site,
                                ScoopDigest256V1 *fingerprint) {
    static const char domain[] = "scoop-stackmap-record-v1";
    size_t locations = 3 + (size_t)record->root_count * 2;
    /* The raw counts are u16. Each location needs at most 20 encoded bytes,
     * each live-out 8; this bound follows the wire shape, not a resource limit. */
    size_t capacity = 8 + sizeof domain - 1 + 4 + 32 + 8 + 32 + 4 + 4 + 4 + 8 + 8 +
                      locations * 20 + 8 + (size_t)record->live_out_count * 8;
    StackmapEncoding encoding = {.bytes = malloc(capacity)};
    if (encoding.bytes == NULL)
        return false;
    encode_u64(&encoding, sizeof domain - 1);
    encode_bytes(&encoding, domain, sizeof domain - 1);
    encode_u32(&encoding, 3);
    encode_bytes(&encoding, site->registration.semantic_id.bytes, 32);
    encode_u64(&encoding, record->safepoint_id);
    encode_bytes(&encoding, site->owner_callable_id.bytes, 32);
    encode_u32(&encoding, site->site_role);
    encode_u32(&encoding, site->root_pair_count);
    encode_u32(&encoding, record->instruction_offset);
    encode_u64(&encoding, record->stack_size);
    encode_u64(&encoding, locations);
    bool valid = true;
    for (size_t index = 0; index < 3; index++) {
        valid = encode_location(&encoding, &record->header[index]) && valid;
    }
    for (size_t index = 0; index < record->root_count; index++) {
        valid = encode_location(&encoding, &record->roots[index].base) && valid;
        valid = encode_location(&encoding, &record->roots[index].derived) && valid;
    }
    encode_u64(&encoding, record->live_out_count);
    for (size_t index = 0; index < record->live_out_count; index++) {
        encode_u32(&encoding, record->live_outs[index].dwarf_register);
        encode_u32(&encoding, record->live_outs[index].size);
    }
    valid = valid &&
            scoop_platform_sha256(encoding.bytes, encoding.length, fingerprint->bytes);
    free(encoding.bytes);
    return valid;
}

static bool same_location(const ScoopStackMapLocation *left,
                          const ScoopStackMapLocation *right) {
    bool left_constant = left->kind == SCOOP_STACKMAP_CONSTANT ||
                         left->kind == SCOOP_STACKMAP_CONSTANT_INDEX;
    bool right_constant = right->kind == SCOOP_STACKMAP_CONSTANT ||
                          right->kind == SCOOP_STACKMAP_CONSTANT_INDEX;
    if (left_constant && right_constant) {
        return left->size == right->size && left->constant == right->constant;
    }
    return left->kind == right->kind && left->size == right->size &&
           left->dwarf_register == right->dwarf_register &&
           left->offset == right->offset;
}

bool scoop_stackmap_same_payload(const ScoopStackMapRecord *left,
                                 const ScoopStackMapRecord *right) {
    if (left->instruction_offset != right->instruction_offset ||
        left->stack_size != right->stack_size ||
        left->root_count != right->root_count ||
        left->live_out_count != right->live_out_count)
        return false;
    for (size_t index = 0; index < 3; index++) {
        if (!same_location(&left->header[index], &right->header[index]))
            return false;
    }
    for (size_t index = 0; index < left->root_count; index++) {
        if (!same_location(&left->roots[index].base, &right->roots[index].base) ||
            !same_location(&left->roots[index].derived, &right->roots[index].derived))
            return false;
    }
    for (size_t index = 0; index < left->live_out_count; index++) {
        if (left->live_outs[index].dwarf_register !=
                right->live_outs[index].dwarf_register ||
            left->live_outs[index].size != right->live_outs[index].size)
            return false;
    }
    return true;
}
