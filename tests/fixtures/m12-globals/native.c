#include <stdint.h>

typedef struct {
    int64_t left;
    int64_t right;
} NativePair;

int64_t native_counter = 30;
_Thread_local int64_t native_tls_counter = 31;
const int64_t native_limit = 32;
NativePair native_pair = {33, 34};
