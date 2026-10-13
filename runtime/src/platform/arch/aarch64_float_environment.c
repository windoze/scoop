#include <fenv.h>
#include <stdint.h>

#include "../../floating.h"

bool scoop_float_init_environment(void) {
    if (fesetenv(FE_DFL_ENV) != 0) return false;
    uint64_t fpcr;
    __asm__ volatile("mrs %0, fpcr" : "=r"(fpcr));
    fpcr &= ~(UINT64_C(1) << 24);
    __asm__ volatile("msr fpcr, %0" : : "r"(fpcr));
    return true;
}
