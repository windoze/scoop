#include <stdint.h>
int8_t round_S8(int8_t value, int8_t (*callback)(int8_t)) { return callback(value); }
int16_t round_S16(int16_t value, int16_t (*callback)(int16_t)) { return callback(value); }
int32_t round_S32(int32_t value, int32_t (*callback)(int32_t)) { return callback(value); }
int64_t round_S64(int64_t value, int64_t (*callback)(int64_t)) { return callback(value); }
uint8_t round_U8(uint8_t value, uint8_t (*callback)(uint8_t)) { return callback(value); }
uint16_t round_U16(uint16_t value, uint16_t (*callback)(uint16_t)) { return callback(value); }
uint32_t round_U32(uint32_t value, uint32_t (*callback)(uint32_t)) { return callback(value); }
uint64_t round_U64(uint64_t value, uint64_t (*callback)(uint64_t)) { return callback(value); }
struct Packet { int8_t head; uint16_t tail; };
struct Packet round_packet(struct Packet value) { return value; }
int8_t integral_global = INT8_MIN;
void replace_integral(int8_t *value) { *value = 0; }
