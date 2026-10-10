#include <stdbool.h>
#include <stdint.h>

struct __attribute__((packed, aligned(8))) Packed {
    bool flag;
    int32_t count;
};
#pragma pack(push, 2)
struct __attribute__((aligned(16))) Envelope {
    int8_t tag;
    struct Packed value;
    int64_t tail;
};
#pragma pack(pop)

struct Envelope m36_envelope(struct Envelope value,
                             struct Envelope (*callback)(struct Envelope)) {
    value.value.count += 5;
    return callback(value);
}

void m36_rewrite(struct Envelope *value) {
    value->value.count += 7;
    value->tail += 8;
}
