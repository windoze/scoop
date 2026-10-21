#include "../native.h"
#include "scoop_rt.h"
#include <assert.h>
#include <pthread.h>
#include <string.h>

float m34_float(uint32_t bits) { float value; memcpy(&value, &bits, 4); return value; }
double m34_double(uint64_t bits) { double value; memcpy(&value, &bits, 8); return value; }
uint32_t m34_float_bits(float value) { uint32_t bits; memcpy(&bits, &value, 4); return bits; }
uint64_t m34_double_bits(double value) { uint64_t bits; memcpy(&bits, &value, 8); return bits; }

Tiny3 m34_tiny(Tiny3 value) {
    assert(value.a == 0x81 && value.b == 0xfe && value.c == 0xff);
    value.b = 7;
    return value;
}
Mixed m34_mixed(Mixed value) { value.n += 9; return value; }
Floats m34_floats(Floats value) {
    assert(m34_float_bits(value.a) == 0xff800123 && m34_float_bits(value.b) == 0x80000000);
    assert(value.c == 3.5f);
    return value;
}
Hfa4 m34_hfa(Hfa4 value) {
    assert(m34_double_bits(value.a) == UINT64_C(0x7ff0000000000123));
    assert(m34_double_bits(value.b) == UINT64_C(0x8000000000000000));
    assert(value.c == 3.5 && value.d == 4.5);
    return value;
}
Big m34_big(Big value) {
    assert(value.a == 1 && value.b == 2 && value.c == 3 && value.d == 4 && value.e == 5);
    value.a = 91;
    value.e = 95;
    return value;
}
Packed m34_packed(Packed value) {
    assert(value.tag == 0x81 && value.x == 1.25);
    value.tag = 7;
    value.x += 0.5;
    return value;
}
Nested m34_nested(Nested value) {
    assert(value.tag == 1 && value.inner.tag == 2 && value.inner.x == 2.25);
    value.inner.x += 1;
    return value;
}
Aligned m34_aligned(Aligned value) {
    assert(m34_float_bits(value.x) == 0xff800123);
    return value;
}
void m34_aligned_address(const Aligned *value) {
    assert((uintptr_t)value % 16 == 0 && m34_float_bits(value->x) == 0xff800123);
}
Padded m34_padded(Padded value) { assert(value.tag == 3); value.n += 4; return value; }
int16_t m34_extend(Hfa4 value, int8_t a, uint16_t b, bool c) {
    assert(value.d == 4.5 && a == -127 && b == 65000 && c);
    return -31000;
}
static int32_t increment(int32_t value) { return value + 1; }
int32_t (*m34_function(void))(int32_t) { return increment; }
Pointers m34_pointers(Pointers value) {
    if (value.data) { assert(value.function && value.function(*value.data) == 43); }
    else { assert(!value.function); }
    return value;
}
Letter m34_letter(Letter value) {
    assert(value.character == 0x754c && value.flag && value.number == -17);
    value.flag = false;
    return value;
}
Mixed m34_safe(Mixed value, Callback callback, void *context) {
    assert(scoop_rt_thread_debug_mode() == SCOOP_THREAD_DEBUG_NATIVE_SAFE);
    value.n += callback((int32_t)value.n, context);
    assert(scoop_rt_thread_debug_mode() == SCOOP_THREAD_DEBUG_NATIVE_SAFE);
    return value;
}

typedef struct { Callback callback; void *context; int32_t input, output; } Work;
static void *run_callback(void *raw) {
    Work *work = raw;
    work->output = work->callback(work->input, work->context);
    return NULL;
}
Big m34_threads(Big value, Callback callback, void *context) {
    assert(scoop_rt_thread_debug_mode() == SCOOP_THREAD_DEBUG_NATIVE_SAFE);
    Work work[2] = {{callback, context, 10, 0}, {callback, context, 20, 0}};
    pthread_t threads[2];
    for (int i = 0; i < 2; ++i) { assert(pthread_create(&threads[i], NULL, run_callback, &work[i]) == 0); }
    for (int i = 0; i < 2; ++i) { assert(pthread_join(threads[i], NULL) == 0); }
    value.e += work[0].output + work[1].output;
    return value;
}
