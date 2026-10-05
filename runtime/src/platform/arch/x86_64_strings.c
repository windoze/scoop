#include "../../string_abi.h"

#if !defined(__x86_64__) || defined(__ILP32__)
#error "the amd64 String result adapters require LP64 x86_64"
#endif

/* Scoop sret places the result pointer in RDI and returns it in RAX. Explicit
 * pointer parameters avoid C's direct aggregate-result classification. */
ScoopStringCharResult *scoop_rt_string_get(ScoopStringCharResult *result,
                                           const ScoopString *value,
                                           int64_t index) {
    scoop_rt_string_get_storage(result, value, index);
    return result;
}

ScoopStringBoundsResult *
scoop_rt_string_slice_bounds(ScoopStringBoundsResult *result,
                             const ScoopString *value, int64_t start,
                             int64_t end) {
    scoop_rt_string_slice_bounds_storage(result, value, start, end);
    return result;
}
