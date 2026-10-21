#ifndef M34_DIRECT_C_NATIVE_H
#define M34_DIRECT_C_NATIVE_H
#include <stdbool.h>
#include <stdint.h>

typedef struct { uint8_t a, b, c; } Tiny3;
typedef struct { double x; int64_t n; } Mixed;
typedef struct { float a, b, c; } Floats;
typedef struct { double a, b, c, d; } Hfa4;
typedef struct { int64_t a, b, c, d, e; } Big;
typedef struct __attribute__((packed)) { uint8_t tag; double x; } Packed;
typedef struct __attribute__((packed)) { uint8_t tag; Packed inner; } Nested;
typedef struct __attribute__((aligned(16))) { float x; } Aligned;
typedef struct { uint8_t tag; int64_t n; } Padded;
typedef struct { int32_t *data; int32_t (*function)(int32_t); } Pointers;
typedef struct { uint32_t character; bool flag; int16_t number; } Letter;
typedef int32_t (*Callback)(int32_t, void *);
#endif
