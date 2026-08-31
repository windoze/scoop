#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

typedef struct {
    int64_t left;
    int64_t right;
} NativePair;

typedef struct __attribute__((packed, aligned(8))) {
    bool flag;
    int64_t value;
} NativePacked;

typedef struct __attribute__((aligned(16))) {
    bool tag;
    NativePacked packed;
    uint64_t tail;
} NativeEnvelope;

_Static_assert(sizeof(NativePacked) == 16, "NativePacked size");
_Static_assert(_Alignof(NativePacked) == 8, "NativePacked alignment");
_Static_assert(offsetof(NativePacked, value) == 1, "NativePacked.value offset");
_Static_assert(sizeof(NativeEnvelope) == 32, "NativeEnvelope size");
_Static_assert(_Alignof(NativeEnvelope) == 16, "NativeEnvelope alignment");
_Static_assert(offsetof(NativeEnvelope, packed) == 8, "NativeEnvelope.packed offset");
_Static_assert(offsetof(NativeEnvelope, tail) == 24, "NativeEnvelope.tail offset");

int64_t native_add(int64_t left, int64_t right) {
    return left + right;
}

uint64_t native_add_unsigned(uint64_t left, uint64_t right) {
    return left + right;
}

uint64_t nativeDefault(uint64_t value) {
    return value * 2;
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

int64_t native_apply_optional(int64_t value, int64_t (*callback)(int64_t)) {
    return callback == NULL ? value : callback(value);
}

NativeEnvelope native_round_trip_envelope(
    NativeEnvelope value,
    NativeEnvelope (*callback)(NativeEnvelope)
) {
    return callback(value);
}
