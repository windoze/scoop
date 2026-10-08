#include "scoop_rt.h"
#include <stdint.h>
#include <stdlib.h>

int32_t m33_native_identity(int32_t value) {
    if (scoop_rt_thread_debug_mode() != SCOOP_THREAD_DEBUG_NATIVE_SAFE) {
        abort();
    }
    return value;
}
