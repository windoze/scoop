#ifndef SCOOP_FLOATING_ABI_H
#define SCOOP_FLOATING_ABI_H

#include "scoop_rt.h"
#include <stddef.h>

/* Exact tagged Option<Float>/Option<Double> storage. Platform adapters return
 * tag and payload bits in integer registers, independently of C struct return. */
typedef struct ScoopFloatResult {
    uint64_t tag;
    float value;
    uint32_t padding;
} ScoopFloatResult;

typedef struct ScoopDoubleResult {
    uint64_t tag;
    double value;
} ScoopDoubleResult;

_Static_assert(sizeof(ScoopFloatResult) == 16 && _Alignof(ScoopFloatResult) == 8 &&
                   offsetof(ScoopFloatResult, value) == 8,
               "Option<Float> storage");
_Static_assert(sizeof(ScoopDoubleResult) == 16 && _Alignof(ScoopDoubleResult) == 8 &&
                   offsetof(ScoopDoubleResult, value) == 8,
               "Option<Double> storage");

void scoop_rt_json_parse_float_storage(ScoopFloatResult *result, const ScoopString *text);
void scoop_rt_json_parse_double_storage(ScoopDoubleResult *result, const ScoopString *text);

#endif
