#include <stdint.h>

extern int32_t m23_helper(int32_t value);
int32_t m23_archived(int32_t value) { return m23_helper(value) + 1; }
