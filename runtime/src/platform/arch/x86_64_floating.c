#include "../../floating_abi.h"
#include <string.h>

#if !defined(__x86_64__) || defined(__ILP32__)
#error "the amd64 floating result adapters require LP64 x86_64"
#endif

typedef struct {
    uint64_t tag;
    uint64_t payload;
} TaggedParts;

TaggedParts scoop_rt_json_parse_float(const ScoopString *text) {
    ScoopFloatResult result;
    scoop_rt_json_parse_float_storage(&result, text);
    TaggedParts parts = {.tag = result.tag, .payload = 0};
    memcpy(&parts.payload, &result.value, sizeof(result.value));
    return parts;
}

TaggedParts scoop_rt_json_parse_double(const ScoopString *text) {
    ScoopDoubleResult result;
    scoop_rt_json_parse_double_storage(&result, text);
    TaggedParts parts = {.tag = result.tag, .payload = 0};
    memcpy(&parts.payload, &result.value, sizeof(result.value));
    return parts;
}
