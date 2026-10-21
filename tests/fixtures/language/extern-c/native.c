#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

typedef struct {
    int32_t left;
    int32_t right;
} NativePair;

typedef struct __attribute__((packed, aligned(8))) {
    bool flag;
    int32_t value;
} NativePacked;

typedef struct __attribute__((aligned(16))) {
    bool tag;
    NativePacked packed;
    uint64_t tail;
} NativeEnvelope;

_Static_assert(sizeof(NativePacked) == 8, "NativePacked size");
_Static_assert(_Alignof(NativePacked) == 8, "NativePacked alignment");
_Static_assert(offsetof(NativePacked, value) == 1, "NativePacked.value offset");
_Static_assert(sizeof(NativeEnvelope) == 32, "NativeEnvelope size");
_Static_assert(_Alignof(NativeEnvelope) == 16, "NativeEnvelope alignment");
_Static_assert(offsetof(NativeEnvelope, packed) == 8, "NativeEnvelope.packed offset");
_Static_assert(offsetof(NativeEnvelope, tail) == 16, "NativeEnvelope.tail offset");

int32_t native_add(int32_t left, int32_t right) {
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

void native_sink(int32_t value) {
    (void)value;
}

int32_t native_apply(int32_t value, int32_t (*callback)(int32_t)) {
    return callback(value);
}

NativePair native_apply_pair(
    NativePair value,
    NativePair (*callback)(NativePair)
) {
    return callback(value);
}

int32_t native_apply_optional(int32_t value, int32_t (*callback)(int32_t)) {
    return callback == NULL ? value : callback(value);
}

NativeEnvelope native_round_trip_envelope(
    NativeEnvelope value,
    NativeEnvelope (*callback)(NativeEnvelope)
) {
    return callback(value);
}
