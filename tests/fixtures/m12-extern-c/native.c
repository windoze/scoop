#include <stdbool.h>
#include <stdint.h>

typedef struct {
    int64_t left;
    int64_t right;
} NativePair;

int64_t native_add(int64_t left, int64_t right) {
    return left + right;
}

bool native_not(bool value) {
    return !value;
}

NativePair native_swap(NativePair value) {
    NativePair result = {value.right, value.left};
    return result;
}

void native_sink(int64_t value) {
    (void)value;
}
