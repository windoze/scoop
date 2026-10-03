#include <stdint.h>
#include <stdlib.h>
#include <string.h>

static unsigned calls;
static void verify(int value) { if (!value) abort(); }
void native_mixed_storage(int8_t i8, int16_t i16, int32_t i32, int64_t i64,
                          uint8_t u8, uint16_t u16, uint32_t u32, uint64_t u64,
                          const void *value) {
    verify(value != NULL);
    uint64_t tag;
    const unsigned char *bytes = value;
    memcpy(&tag, bytes, sizeof(tag));
    verify(tag == calls);
    if (tag == 0) {
        verify(bytes[8] == 7 && bytes[9] == 1);
    } else if (tag == 1) {
        const void *text;
        memcpy(&text, bytes + 16, sizeof(text));
        verify(text != NULL);
    } else {
        verify(tag == 2);
    }
    verify(i8 == -7 && i16 == -300 && i32 == -123456 && i64 == -INT64_C(5000000000));
    verify(u8 == 250 && u16 == 60000 && u32 == UINT32_C(4000000000) && u64 == UINT64_C(9000000000));
    calls++;
}
void verify_finished(void) { verify(calls == 3); }

/* The Scoop byval aggregate occupies 24 bytes at the incoming SP; the
 * eight scalar arguments occupy x0-x7. Expose its storage as C argument 9. */
__asm__(
    ".text\n"
    ".globl _native_mixed\n"
    ".p2align 2\n"
    "_native_mixed:\n"
    "mov x9, sp\n"
    "sub sp, sp, #32\n"
    "stp x29, x30, [sp, #16]\n"
    "mov x29, sp\n"
    "str x9, [sp]\n"
    "bl _native_mixed_storage\n"
    "ldp x29, x30, [sp, #16]\n"
    "add sp, sp, #32\n"
    "ret\n");
