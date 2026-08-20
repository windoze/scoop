#include <assert.h>
#include <stdbool.h>
#include <stdint.h>

int8_t m33_s8(int8_t value) {
    assert(value == -101);
    return -117;
}
uint8_t m33_u8(uint8_t value) {
    assert(value == 201);
    return 239;
}
int16_t m33_s16(int16_t value) {
    assert(value == -30001);
    return -32001;
}
uint16_t m33_u16(uint16_t value) {
    assert(value == 50001);
    return 65521;
}
int32_t m33_s32(int32_t value) {
    assert(value == -123456789);
    return INT32_MIN + 1;
}
uint32_t m33_u32(uint32_t value) {
    assert(value == UINT32_C(0xfedcba98));
    return UINT32_MAX - 1;
}
int64_t m33_s64(int64_t value) {
    assert(value == -INT64_C(98765432101234567));
    return INT64_MIN + 3;
}
uint64_t m33_u64(uint64_t value) {
    assert(value == UINT64_C(0xfedcba9876543210));
    return UINT64_MAX - 14;
}
bool m33_boolean(bool value) { return !value; }
float m33_float(float value) {
    assert(value == 1.25f);
    return -2.75f;
}
double m33_double(double value) {
    assert(value == 0.5);
    return -123.25;
}

/* The chosen native symbol must not become LLVM's abs builtin. */
#ifdef __APPLE__
int m33_selected_abs(int value) __asm__("_abs");
#else
int m33_selected_abs(int value) __asm__("abs");
#endif
int m33_selected_abs(int value) { return value + 10; }

double m33_mixed(int64_t a0, double b0, int64_t a1, double b1, int64_t a2, double b2, int64_t a3,
                 double b3, int64_t a4, double b4, int64_t a5, double b5, int64_t a6, double b6,
                 int64_t a7, double b7, int64_t a8, double b8, int64_t a9, double b9, int8_t tag,
                 bool flag, uint16_t tail) {
    return a0 + 2 * a1 + 3 * a2 + 4 * a3 + 5 * a4 + 6 * a5 + 7 * a6 + 8 * a7 + 9 * a8 + 10 * a9 +
           b0 + 2 * b1 + 3 * b2 + 4 * b3 + 5 * b4 + 6 * b5 + 7 * b6 + 8 * b7 + 9 * b8 + 10 * b9 +
           tag + (flag ? 7 : 0) + tail;
}

typedef struct Item {
    int32_t value;
    double extra;
} Item;
typedef int32_t (*NativeFunction)(int32_t);
static int32_t record;
static int32_t transform(int32_t value) { return value * 3 + 1; }
uint64_t m33_word(uint64_t value) { return value; }
uint64_t m33_handle_word(uint64_t value) { return value; }
uint32_t m33_char(uint32_t value) {
    assert(value == UINT32_C(30028));
    return value;
}
int32_t *m33_pointer(int32_t *value) { return value; }
int32_t *m33_safe_pointer(int32_t *value) { return value; }
Item *m33_item_pointer(Item *value) { return value; }
NativeFunction m33_function(void) { return transform; }
NativeFunction m33_optional_function(NativeFunction value) { return value; }
int32_t m33_invoke(NativeFunction value, int32_t argument) { return value(argument); }
void m33_record(int32_t value) { record += value; }
int32_t m33_recorded(void) { return record; }
