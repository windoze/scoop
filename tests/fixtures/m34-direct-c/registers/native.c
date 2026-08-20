#include "../native.h"
#include <assert.h>

Mixed m34_gp_full(int64_t a, int64_t b, int64_t c, int64_t d, int64_t e, int64_t f,
                  Mixed value, double tail, int16_t small) {
    assert(a + b + c + d + e + f == 21 && value.x == 1.25 && value.n == 42);
    assert(tail == 2.5 && small == -31000);
    value.n += 21;
    return value;
}
Mixed m34_fp_full(double a, double b, double c, double d, double e, double f,
                  double g, double h, Mixed value, int64_t tail) {
    assert(a + b + c + d + e + f + g + h == 36 && value.x == 1.25 && value.n == 42);
    assert(tail == 73);
    value.n += tail;
    return value;
}
Big m34_sret_full(int64_t a, int64_t b, int64_t c, int64_t d, int64_t e,
                  Mixed value, double tail) {
    assert(a + b + c + d + e == 15 && value.x == 1.25 && value.n == 42 && tail == 2.5);
    return (Big){1, 2, 3, 4, value.n + 15};
}
Mixed m34_gp_boundary(int64_t a, int64_t b, int64_t c, int64_t d, int64_t e,
                      int64_t f, int64_t g, Mixed value, int64_t tail) {
    assert(a + b + c + d + e + f + g == 28 && value.x == 1.25 && value.n == 42);
    assert(tail == 73);
    return value;
}
Floats m34_fp_boundary(double a, double b, double c, double d, double e,
                       double f, double g, Floats value, double tail) {
    assert(a + b + c + d + e + f + g == 28 && tail == 73.5);
    assert(value.a == 1.5f && value.b == 2.5f && value.c == 3.5f);
    return value;
}
Hfa4 m34_mixed_stack(int64_t a, int64_t b, int64_t c, int64_t d, int64_t e,
                     int64_t f, Hfa4 hfa, Packed packed, Mixed mixed, uint8_t tail) {
    assert(a + b + c + d + e + f == 21 && tail == 255);
    assert(hfa.a == 1.25 && hfa.b == 2.25 && hfa.c == 3.25 && hfa.d == 4.25);
    assert(packed.tag == 129 && packed.x == 5.25 && mixed.x == 6.25 && mixed.n == 73);
    return hfa;
}
