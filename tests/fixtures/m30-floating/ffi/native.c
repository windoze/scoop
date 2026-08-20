#include <stdint.h>
#include <string.h>

typedef struct { float first, second; } Singles;
typedef struct { double first, second; } Doubles;
typedef struct { float small; int32_t tag; double wide; } Mixed;
float global_single = 0.25f;
const double global_wide = -0.0;
_Thread_local float tls_single = 0.5f;
_Thread_local double tls_wide = -0.0;

float float_from_bits(uint32_t bits) { float v; memcpy(&v, &bits, 4); return v; }
double double_from_bits(uint64_t bits) { double v; memcpy(&v, &bits, 8); return v; }
uint32_t float_bits(float value) { uint32_t v; memcpy(&v, &value, 4); return v; }
uint64_t double_bits(double value) { uint64_t v; memcpy(&v, &value, 8); return v; }
float scalar_single(float value) { return value; }
double scalar_wide(double value) { return value; }
Singles round_singles(Singles value) { return value; }
Doubles round_doubles(Doubles value) { return value; }
Mixed round_mixed(Mixed value) { return value; }
float pointer_single(float *value) { return *value; }
double pointer_wide(double *value) { return *value; }
int32_t tls_matches(uint32_t small, uint64_t wide) {
    return float_bits(tls_single) == small && double_bits(tls_wide) == wide;
}

typedef float (*SingleFn)(float);
typedef double (*WideFn)(double);
SingleFn native_single(void) { return scalar_single; }
WideFn native_wide(void) { return scalar_wide; }
float invoke_single(SingleFn callback, float value) { return callback(value); }
double invoke_wide(WideFn callback, double value) { return callback(value); }
float invoke_managed(float (*callback)(float, void *), void *context, float value) {
    return callback(value, context);
}
