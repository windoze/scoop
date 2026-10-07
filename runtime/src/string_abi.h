#ifndef SCOOP_STRING_ABI_H
#define SCOOP_STRING_ABI_H

#include "scoop_rt.h"
#include <stddef.h>

/* Exact core Option<Char> and Option<(Long, Long)> value storage. Platform
 * adapters receive Scoop's indirect result address using the target ABI. */
typedef struct ScoopStringCharResult {
    uint64_t tag;
    uint32_t value;
    uint32_t padding;
} ScoopStringCharResult;

typedef struct ScoopStringBoundsResult {
    uint64_t tag;
    int64_t start;
    int64_t end;
} ScoopStringBoundsResult;

typedef struct ScoopStringDecodeResult {
    const ScoopString *value;
    int64_t invalid_offset;
} ScoopStringDecodeResult;

_Static_assert(sizeof(ScoopStringCharResult) == 16 && _Alignof(ScoopStringCharResult) == 8 &&
                   offsetof(ScoopStringCharResult, value) == 8,
               "Option<Char> storage");
_Static_assert(sizeof(ScoopStringBoundsResult) == 24 && _Alignof(ScoopStringBoundsResult) == 8 &&
                   offsetof(ScoopStringBoundsResult, start) == 8 &&
                   offsetof(ScoopStringBoundsResult, end) == 16,
               "Option<(Long, Long)> storage");
_Static_assert(sizeof(ScoopStringDecodeResult) == 16 && _Alignof(ScoopStringDecodeResult) == 8 &&
                   offsetof(ScoopStringDecodeResult, invalid_offset) == 8,
               "(String?, Long) storage");

void scoop_rt_string_get_storage(ScoopStringCharResult *result, const ScoopString *value,
                                 int64_t index);
void scoop_rt_string_slice_bounds_storage(ScoopStringBoundsResult *result, const ScoopString *value,
                                          int64_t start, int64_t end);

void scoop_rt_string_decode_utf8_storage(ScoopStringDecodeResult *result,
                                         const unsigned char *bytes, int64_t length);
const ScoopString *scoop_rt_string_decode_utf8_lossy(const unsigned char *bytes, int64_t length);

#endif
