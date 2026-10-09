#include "../../string_abi.h"

#if !defined(__x86_64__) || defined(__ILP32__)
#error "the amd64 String result adapters require LP64 x86_64"
#endif

/* Larger or GC-bearing Scoop results use RDI for sret and return it in RAX. */
ScoopStringBoundsResult *scoop_rt_string_slice_bounds(ScoopStringBoundsResult *result,
                                                      const ScoopString *value, int64_t start,
                                                      int64_t end) {
    scoop_rt_string_slice_bounds_storage(result, value, start, end);
    return result;
}

ScoopStringDecodeResult *scoop_rt_string_decode_utf8(ScoopStringDecodeResult *result,
                                                     const unsigned char *bytes, int64_t length) {
    scoop_rt_string_decode_utf8_storage(result, bytes, length);
    return result;
}
