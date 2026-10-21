#include <stdint.h>

typedef struct {
    int32_t left;
    int32_t right;
} NativePair;

int32_t native_counter = 30;
_Thread_local int32_t native_tls_counter = 31;
const int32_t native_limit = 32;
NativePair native_pair = {33, 34};
