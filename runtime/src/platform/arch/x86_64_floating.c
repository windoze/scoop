#include "../../floating_abi.h"

#if !defined(__x86_64__) || defined(__ILP32__)
#error "the amd64 floating result adapters require LP64 x86_64"
#endif

/* Scoop sret uses RDI for the result pointer and returns it in RAX. */
ScoopFloatResult *scoop_rt_json_parse_float(ScoopFloatResult *result,
                                          const ScoopString *text) {
    scoop_rt_json_parse_float_storage(result, text);
    return result;
}

ScoopDoubleResult *scoop_rt_json_parse_double(ScoopDoubleResult *result,
                                            const ScoopString *text) {
    scoop_rt_json_parse_double_storage(result, text);
    return result;
}
