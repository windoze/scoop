#include <stdint.h>

int32_t m23_global = 2;
int32_t m23_target(int32_t value) { return value + 1; }
int32_t (*m23_pointer)(int32_t) = m23_target;
static int32_t local_call(int32_t value) { return m23_target(value); }
int32_t m23_final(int32_t value) {
    return local_call(value) + m23_global + m23_pointer(value);
}
