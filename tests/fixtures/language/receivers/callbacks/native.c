#include <stdint.h>

int32_t m37_receiver_callback(int32_t (*callback)(int32_t, void *),
                              void *context, int32_t value) {
    return callback(value, context);
}
