#include <stdint.h>

typedef struct {
    uint8_t prefix;
    uint32_t character;
    int64_t tail;
} Packet;

uint32_t round_character(uint32_t value) { return value; }
Packet round_packet(Packet value) { return value; }
void replace_character(uint32_t *value) { *value = UINT32_C(30028); }
