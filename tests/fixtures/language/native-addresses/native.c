#include <stdint.h>

int64_t m23_call_scalar(int64_t (*callback)(int64_t), int64_t value) { return callback(value); }

struct NativeWide {
    int64_t first;
    int64_t second;
    int64_t third;
};
int64_t m23_call_wide(struct NativeWide (*callback)(struct NativeWide)) {
    struct NativeWide value = {17, -29, 54};
    struct NativeWide result = callback(value);
    return result.first + result.second + result.third;
}

int64_t m23_call_sink(void (*callback)(int64_t *)) {
    int64_t value = 0;
    callback(&value);
    return value;
}
