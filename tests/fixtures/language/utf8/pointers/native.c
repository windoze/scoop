#include <stdint.h>

const uint8_t *native_bytes(int32_t kind) {
    static const uint8_t valid[] = {65, 0xe7, 0x95, 0x8c, 0, 66};
    static const uint8_t invalid[] = {65, 0xe1, 0x80};
    return kind == 0 ? valid : invalid;
}
