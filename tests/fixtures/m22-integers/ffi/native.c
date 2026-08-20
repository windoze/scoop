#include <stdint.h>

typedef struct {
    int8_t signed8;
    int16_t signed16;
    int32_t signed32;
    int64_t signed64;
    uint8_t unsigned8;
    uint16_t unsigned16;
    uint32_t unsigned32;
    uint64_t unsigned64;
} FixedWidths;

int8_t round_i8(int8_t value) { return value; }
int16_t round_i16(int16_t value) { return value; }
int32_t round_i32(int32_t value) { return value; }
int64_t round_i64(int64_t value) { return value; }
uint8_t round_u8(uint8_t value) { return value; }
uint16_t round_u16(uint16_t value) { return value; }
uint32_t round_u32(uint32_t value) { return value; }
uint64_t round_u64(uint64_t value) { return value; }

FixedWidths transform_widths(FixedWidths value) {
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

int8_t native_signed8 = -7;
int16_t native_signed16 = INT16_MIN;
int32_t native_signed32 = INT32_MIN;
int64_t native_signed64 = INT64_MIN;
uint8_t native_unsigned8 = UINT8_MAX;
uint16_t native_unsigned16 = UINT16_MAX;
uint32_t native_unsigned32 = UINT32_MAX;
uint64_t native_unsigned64 = UINT64_MAX;

_Thread_local int8_t native_tls_signed8 = -17;
_Thread_local int16_t native_tls_signed16 = -1700;
_Thread_local int32_t native_tls_signed32 = -170000;
_Thread_local int64_t native_tls_signed64 = -INT64_C(17000000000);
_Thread_local uint8_t native_tls_unsigned8 = UINT8_C(17);
_Thread_local uint16_t native_tls_unsigned16 = UINT16_C(1700);
_Thread_local uint32_t native_tls_unsigned32 = UINT32_C(170000);
_Thread_local uint64_t native_tls_unsigned64 = UINT64_C(17000000000);

uint8_t verify_written_globals(void) {
    uint8_t verified = 0;
    if (native_signed8 == INT8_MAX) {
        verified |= UINT8_C(1);
    }
    if (native_signed16 == INT16_MAX) {
        verified |= UINT8_C(2);
    }
    if (native_signed32 == INT32_MAX) {
        verified |= UINT8_C(4);
    }
    if (native_signed64 == INT64_MAX) {
        verified |= UINT8_C(8);
    }
    if (native_unsigned8 == UINT8_MAX - UINT8_C(1)) {
        verified |= UINT8_C(16);
    }
    if (native_unsigned16 == UINT16_MAX - UINT16_C(1)) {
        verified |= UINT8_C(32);
    }
    if (native_unsigned32 == UINT32_MAX - UINT32_C(1)) {
        verified |= UINT8_C(64);
    }
    if (native_unsigned64 == UINT64_MAX - UINT64_C(1)) {
        verified |= UINT8_C(128);
    }
    return verified;
}

uint8_t verify_written_tls(void) {
    uint8_t verified = 0;
    if (native_tls_signed8 == INT8_C(126)) {
        verified |= UINT8_C(1);
    }
    if (native_tls_signed16 == INT16_C(32766)) {
        verified |= UINT8_C(2);
    }
    if (native_tls_signed32 == INT32_C(2147483646)) {
        verified |= UINT8_C(4);
    }
    if (native_tls_signed64 == INT64_C(9223372036854775806)) {
        verified |= UINT8_C(8);
    }
    if (native_tls_unsigned8 == UINT8_C(253)) {
        verified |= UINT8_C(16);
    }
    if (native_tls_unsigned16 == UINT16_C(65533)) {
        verified |= UINT8_C(32);
    }
    if (native_tls_unsigned32 == UINT32_C(4294967293)) {
        verified |= UINT8_C(64);
    }
    if (native_tls_unsigned64 == UINT64_C(18446744073709551613)) {
        verified |= UINT8_C(128);
    }
    return verified;
}

uint16_t apply_u16(uint16_t value, uint16_t (*callback)(uint16_t)) {
    return callback(value);
}

uint8_t verify_widths_callback(FixedWidths (*callback)(FixedWidths)) {
    FixedWidths input = {
        -101,
        -20002,
        -INT32_C(300000003),
        -INT64_C(4000000000000000004),
        UINT8_C(201),
        UINT16_C(50005),
        UINT32_C(3000000006),
        UINT64_C(16000000000000000007),
    };
    FixedWidths output = callback(input);
    uint8_t verified = 0;
    if (output.signed8 == input.signed8) {
        verified |= UINT8_C(1);
    }
    if (output.signed16 == input.signed16) {
        verified |= UINT8_C(2);
    }
    if (output.signed32 == input.signed32) {
        verified |= UINT8_C(4);
    }
    if (output.signed64 == input.signed64) {
        verified |= UINT8_C(8);
    }
    if (output.unsigned8 == input.unsigned8) {
        verified |= UINT8_C(16);
    }
    if (output.unsigned16 == input.unsigned16) {
        verified |= UINT8_C(32);
    }
    if (output.unsigned32 == input.unsigned32) {
        verified |= UINT8_C(64);
    }
    if (output.unsigned64 == input.unsigned64) {
        verified |= UINT8_C(128);
    }
    return verified;
}
