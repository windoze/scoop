#include <stdint.h>
#include <string.h>
float float_from_bits(uint32_t bits) { float v; memcpy(&v, &bits, 4); return v; }
double double_from_bits(uint64_t bits) { double v; memcpy(&v, &bits, 8); return v; }
uint32_t float_bits(float value) { uint32_t v; memcpy(&v, &value, 4); return v; }
uint64_t double_bits(double value) { uint64_t v; memcpy(&v, &value, 8); return v; }
