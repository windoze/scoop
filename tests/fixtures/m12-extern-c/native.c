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

int64_t native_apply(int64_t value, int64_t (*callback)(int64_t)) {
    return callback(value);
}

NativePair native_apply_pair(
    NativePair value,
    NativePair (*callback)(NativePair)
) {
    return callback(value);
}
