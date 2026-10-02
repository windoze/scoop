#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

typedef struct __attribute__((packed, aligned(8))) Packed {
    bool flag;
    int32_t value;
} Packed;

#pragma pack(push, 2)
typedef struct __attribute__((aligned(16))) Envelope {
    uint16_t tag;
    Packed packed;
    int64_t tail;
} Envelope;
#pragma pack(pop)

typedef struct Widths {
    int8_t signed8;
    int16_t signed16;
    int32_t signed32;
    int64_t signed64;
    uint8_t unsigned8;
    uint16_t unsigned16;
    uint32_t unsigned32;
    uint64_t unsigned64;
} Widths;

_Static_assert(sizeof(Packed) == 8 && _Alignof(Packed) == 8 &&
               offsetof(Packed, value) == 1, "packed layout");
_Static_assert(sizeof(Envelope) == 32 && _Alignof(Envelope) == 16 &&
               offsetof(Envelope, packed) == 2 && offsetof(Envelope, tail) == 10,
               "nested packed and aligned layout");
_Static_assert(sizeof(Widths) == 32 && _Alignof(Widths) == 8 &&
               offsetof(Widths, signed16) == 2 && offsetof(Widths, signed32) == 4 &&
               offsetof(Widths, signed64) == 8 && offsetof(Widths, unsigned8) == 16 &&
               offsetof(Widths, unsigned16) == 18 && offsetof(Widths, unsigned32) == 20 &&
               offsetof(Widths, unsigned64) == 24, "fixed-width field layout");

int8_t m23_i8(int8_t value) { return value; }
int16_t m23_i16(int16_t value) { return value; }
int32_t m23_i32(int32_t value) { return value; }
int64_t m23_i64(int64_t value) { return value; }
uint8_t m23_u8(uint8_t value) { return value; }
uint16_t m23_u16(uint16_t value) { return value; }
uint32_t m23_u32(uint32_t value) { return value; }
uint64_t m23_u64(uint64_t value) { return value; }

Packed m23_packed(Packed value) {
    value.flag = !value.flag;
    value.value += 1;
    return value;
}

Widths m23_widths(Widths value) {
    value.signed8 -= 1;
    value.signed16 -= 2;
    value.signed32 -= 3;
    value.signed64 -= 4;
    value.unsigned8 += 1;
    value.unsigned16 += 2;
    value.unsigned32 += 3;
    value.unsigned64 += 4;
    return value;
}

Envelope m23_envelope(Envelope value, Envelope (*callback)(Envelope)) {
    value.tail += 1;
    return callback(value);
}

int32_t m23_pointer(int32_t *pointer, const int32_t *other,
                    int32_t (*callback)(int32_t)) {
    int32_t increment = other == NULL ? 2 : *other;
    *pointer += increment;
    return callback == NULL ? *pointer : callback(*pointer);
}
