/* Ryu supplies shortest digits; this adapter fixes Scoop's textual shape. */
#include <fenv.h>
#include <string.h>

#if defined(__x86_64__)
#include <xmmintrin.h>
#endif

#include "floating.h"
#include "ryu/ryu.h"
#include "scoop_rt.h"
#include "value_shape.h"

extern const ScoopTypeDescriptor scoop_td_String;

bool scoop_float_init_environment(void) {
    if (fesetenv(FE_DFL_ENV) != 0) return false;
#if defined(__x86_64__)
    /* SSE has separate flush-to-zero and denormals-are-zero controls. */
    _mm_setcsr(_mm_getcsr() & ~UINT32_C(0x8040));
#elif defined(__aarch64__)
    uint64_t fpcr;
    __asm__ volatile("mrs %0, fpcr" : "=r"(fpcr));
    fpcr &= ~(UINT64_C(1) << 24);
    __asm__ volatile("msr fpcr, %0" : : "r"(fpcr));
#else
#error "floating runtime requires a supported 64-bit architecture"
#endif
    return true;
}

static size_t format_shortest(const char *raw, size_t length, char *out) {
    const char *exponent_start = memchr(raw, 'E', length);
    if (exponent_start == NULL) {
        memcpy(out, raw, length);
        return length;
    }
    size_t written = 0;
    size_t start = 0;
    if (raw[0] == '-') {
        out[written++] = '-';
        start = 1;
    }
    char digits[17];
    size_t count = 0;
    for (const char *p = raw + start; p < exponent_start; ++p) {
        if (*p != '.') digits[count++] = *p;
    }
    const char *p = exponent_start + 1;
    int sign = 1;
    if (*p == '-') { sign = -1; ++p; }
    if (*p == '+') ++p;
    int exponent = 0;
    for (; p < raw + length; ++p) exponent = exponent * 10 + (*p - '0');
    exponent *= sign;

    if (exponent >= -3 && exponent < 7) {
        int point = exponent + 1;
        if (point <= 0) {
            out[written++] = '0';
            out[written++] = '.';
            for (int i = point; i < 0; ++i) out[written++] = '0';
            memcpy(out + written, digits, count);
            return written + count;
        }
        for (int i = 0; i < point; ++i)
            out[written++] = (size_t)i < count ? digits[i] : '0';
        out[written++] = '.';
        if ((size_t)point >= count) out[written++] = '0';
        else {
            memcpy(out + written, digits + point, count - (size_t)point);
            written += count - (size_t)point;
        }
        return written;
    }

    out[written++] = digits[0];
    out[written++] = '.';
    if (count == 1) out[written++] = '0';
    else {
        memcpy(out + written, digits + 1, count - 1);
        written += count - 1;
    }
    out[written++] = 'e';
    if (exponent < 0) { out[written++] = '-'; exponent = -exponent; }
    char reversed[3];
    size_t exponent_length = 0;
    do {
        reversed[exponent_length++] = (char)('0' + exponent % 10);
        exponent /= 10;
    } while (exponent != 0);
    while (exponent_length != 0) out[written++] = reversed[--exponent_length];
    return written;
}

static const ScoopString *shortest_string(const char *raw, size_t length) {
    char text[32];
    size_t size = format_shortest(raw, length, text);
    ScoopString *result = scoop_rt_alloc(
        &scoop_td_String, scoop_shape_allocation_size(&scoop_td_String, size));
    result->len = size;
    memcpy(result->data, text, size);
    return result;
}

const ScoopString *scoop_rt_float_to_string(float value) {
    char raw[32];
    int length = f2s_buffered_n(value, raw);
    return shortest_string(raw, (size_t)length);
}

const ScoopString *scoop_rt_double_to_string(double value) {
    char raw[32];
    int length = d2s_buffered_n(value, raw);
    return shortest_string(raw, (size_t)length);
}
