#ifndef SCOOP_RT_LSDA_READER_H
#define SCOOP_RT_LSDA_READER_H

#include "../eh_internal.h"
#include <stdbool.h>

typedef enum ScoopEhReadStatus {
    SCOOP_EH_READ_OK,
    SCOOP_EH_READ_TRUNCATED,
    SCOOP_EH_READ_OVERFLOW,
} ScoopEhReadStatus;

typedef struct ScoopEhCursor {
    ScoopEhByteRange range;
    size_t offset;
} ScoopEhCursor;

static inline ScoopEhDecodeResult scoop_eh_result(ScoopEhDecodeKind kind,
                                                  uintptr_t landing_pad,
                                                  uint32_t selector) {
    return (ScoopEhDecodeResult){
        .kind = kind,
        .landing_pad = landing_pad,
        .selector = selector,
        .reason = SCOOP_EH_REASON_NONE,
        .error_offset = 0,
    };
}

static inline ScoopEhDecodeResult scoop_eh_failure(ScoopEhDecodeKind kind,
                                                   ScoopEhDecodeReason reason,
                                                   size_t offset) {
    return (ScoopEhDecodeResult){
        .kind = kind,
        .landing_pad = 0,
        .selector = 0,
        .reason = reason,
        .error_offset = offset,
    };
}

static inline ScoopEhReadStatus scoop_eh_read_u8(ScoopEhCursor *cursor,
                                                 uint8_t *value) {
    if (cursor->offset >= cursor->range.length) {
        return SCOOP_EH_READ_TRUNCATED;
    }
    *value = cursor->range.bytes[cursor->offset++];
    return SCOOP_EH_READ_OK;
}

static inline ScoopEhReadStatus scoop_eh_read_uleb128(ScoopEhCursor *cursor,
                                                      uint64_t *value) {
    uint64_t result = 0;
    for (unsigned index = 0; index < 10; index++) {
        uint8_t byte = 0;
        ScoopEhReadStatus status = scoop_eh_read_u8(cursor, &byte);
        if (status != SCOOP_EH_READ_OK) {
            return status;
        }
        uint64_t payload = (uint64_t)(byte & UINT8_C(0x7f));
        if (index == 9) {
            if ((byte & UINT8_C(0x80)) != 0 || payload > 1) {
                return SCOOP_EH_READ_OVERFLOW;
            }
            result |= payload << 63;
            *value = result;
            return SCOOP_EH_READ_OK;
        }
        result |= payload << (index * 7);
        if ((byte & UINT8_C(0x80)) == 0) {
            *value = result;
            return SCOOP_EH_READ_OK;
        }
    }
    return SCOOP_EH_READ_OVERFLOW;
}

static inline int64_t scoop_eh_signed_bits(uint64_t bits) {
    if (bits <= (uint64_t)INT64_MAX) {
        return (int64_t)bits;
    }
    uint64_t magnitude = (~bits) + UINT64_C(1);
    if (magnitude == (UINT64_C(1) << 63)) {
        return INT64_MIN;
    }
    return -(int64_t)magnitude;
}

static inline ScoopEhReadStatus scoop_eh_read_sleb128(ScoopEhCursor *cursor,
                                                      int64_t *value) {
    uint64_t result = 0;
    for (unsigned index = 0; index < 10; index++) {
        uint8_t byte = 0;
        ScoopEhReadStatus status = scoop_eh_read_u8(cursor, &byte);
        if (status != SCOOP_EH_READ_OK) {
            return status;
        }
        uint64_t payload = (uint64_t)(byte & UINT8_C(0x7f));
        if (index == 9) {
            if ((byte & UINT8_C(0x80)) != 0 ||
                (payload != 0 && payload != UINT64_C(0x7f))) {
                return SCOOP_EH_READ_OVERFLOW;
            }
            if (payload == UINT64_C(0x7f)) {
                result |= UINT64_C(1) << 63;
            }
            *value = scoop_eh_signed_bits(result);
            return SCOOP_EH_READ_OK;
        }
        result |= payload << (index * 7);
        if ((byte & UINT8_C(0x80)) == 0) {
            unsigned shift = (index + 1) * 7;
            if ((byte & UINT8_C(0x40)) != 0) {
                result |= UINT64_MAX << shift;
            }
            *value = scoop_eh_signed_bits(result);
            return SCOOP_EH_READ_OK;
        }
    }
    return SCOOP_EH_READ_OVERFLOW;
}

static inline ScoopEhDecodeReason
scoop_eh_read_reason(ScoopEhReadStatus status) {
    switch (status) {
    case SCOOP_EH_READ_OK:
        return SCOOP_EH_REASON_NONE;
    case SCOOP_EH_READ_TRUNCATED:
        return SCOOP_EH_REASON_TRUNCATED;
    case SCOOP_EH_READ_OVERFLOW:
        return SCOOP_EH_REASON_INTEGER_OVERFLOW;
    }
    return SCOOP_EH_REASON_INTEGER_OVERFLOW;
}

static inline bool scoop_eh_size_from_u64(uint64_t value, size_t *result) {
    if (value > (uint64_t)SIZE_MAX) {
        return false;
    }
    *result = (size_t)value;
    return true;
}

static inline bool scoop_eh_checked_size_add(size_t left, size_t right,
                                             size_t *result) {
    if (right > SIZE_MAX - left) {
        return false;
    }
    *result = left + right;
    return true;
}

static inline bool scoop_eh_checked_address_add(uintptr_t base, uint64_t offset,
                                                uintptr_t *result) {
    if (offset > (uint64_t)(UINTPTR_MAX - base)) {
        return false;
    }
    *result = base + (uintptr_t)offset;
    return true;
}

#endif /* SCOOP_RT_LSDA_READER_H */
