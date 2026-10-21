#include <stdint.h>

int64_t m23_call_scalar(int64_t (*callback)(int64_t), int64_t value) { return callback(value); }
