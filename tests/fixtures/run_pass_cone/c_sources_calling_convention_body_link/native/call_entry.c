#include <stdint.h>

extern int64_t scoop_native_double_for_c(int64_t value);

int64_t call_scoop_native_double_from_c(int64_t value) {
    return scoop_native_double_for_c(value);
}
