#include <fenv.h>
#include <stdint.h>
#include <xmmintrin.h>

#include "../../floating.h"

bool scoop_float_init_environment(void) {
    if (fesetenv(FE_DFL_ENV) != 0) return false;
    /* SSE has separate flush-to-zero and denormals-are-zero controls. */
    _mm_setcsr(_mm_getcsr() & ~UINT32_C(0x8040));
    return true;
}
