#include <stdint.h>

int16_t m37_round_code(int16_t value, int16_t (*callback)(int16_t)) {
    return callback(value);
}
