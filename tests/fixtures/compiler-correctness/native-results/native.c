#include "scoop_rt.h"

#include <stdint.h>

static int64_t calls;

void m36_result_touch(void) { calls++; }
void m36_result_empty(void) { calls++; }
int64_t m36_result_number(void) { return calls; }

const ScoopString *m36_result_echo(const ScoopString *value) {
    void *root = (void *)value;
    void **slots[] = {&root};
    ScoopNativeRootFrame frame;
    scoop_rt_push_native_roots(&frame, slots, 1);
    scoop_runtime_gc_collect();
    scoop_rt_pop_native_roots(&frame);
    return root;
}

typedef struct Wide {
    int64_t first, second, third;
} Wide;

typedef struct BoxedWide {
    const ScoopString *text;
    int64_t left, right;
} BoxedWide;

Wide *m36_result_wide_storage(Wide *result) {
    *result = (Wide){1, 2, 3};
    return result;
}

BoxedWide *m36_result_boxed_storage(BoxedWide *result, const ScoopString *value) {
    void *root = (void *)value;
    void **slots[] = {&root};
    ScoopNativeRootFrame frame;
    scoop_rt_push_native_roots(&frame, slots, 1);
    scoop_runtime_gc_collect();
    *result = (BoxedWide){root, 10, 20};
    scoop_runtime_gc_collect();
    scoop_rt_pop_native_roots(&frame);
    return result;
}

/* Adapt Scoop's sret register to the ordinary C helper's first parameter. */
#if defined(__APPLE__) && defined(__aarch64__)
__asm__(".text\n"
        ".globl _m36_result_wide\n"
        ".p2align 2\n"
        "_m36_result_wide:\n"
        "mov x0, x8\n"
        "b _m36_result_wide_storage\n"
        ".globl _m36_result_boxed\n"
        ".p2align 2\n"
        "_m36_result_boxed:\n"
        "mov x1, x0\n"
        "mov x0, x8\n"
        "b _m36_result_boxed_storage\n");
#elif defined(__x86_64__) && (defined(__linux__) || defined(__APPLE__))
#if defined(__APPLE__)
#define SCOOP_TEST_ASM_SYMBOL(name) "_" name
#define SCOOP_TEST_ASM_ELF(directive) ""
#else
#define SCOOP_TEST_ASM_SYMBOL(name) name
#define SCOOP_TEST_ASM_ELF(directive) directive
#endif
__asm__(".text\n"
        ".globl " SCOOP_TEST_ASM_SYMBOL("m36_result_wide") "\n"
        SCOOP_TEST_ASM_ELF(".type m36_result_wide,@function\n")
        SCOOP_TEST_ASM_SYMBOL("m36_result_wide") ":\n"
        "jmp " SCOOP_TEST_ASM_SYMBOL("m36_result_wide_storage") "\n"
        SCOOP_TEST_ASM_ELF(".size m36_result_wide,.-m36_result_wide\n")
        ".globl " SCOOP_TEST_ASM_SYMBOL("m36_result_boxed") "\n"
        SCOOP_TEST_ASM_ELF(".type m36_result_boxed,@function\n")
        SCOOP_TEST_ASM_SYMBOL("m36_result_boxed") ":\n"
        "jmp " SCOOP_TEST_ASM_SYMBOL("m36_result_boxed_storage") "\n"
        SCOOP_TEST_ASM_ELF(".size m36_result_boxed,.-m36_result_boxed\n"));
#elif defined(_WIN32) && defined(__x86_64__)
__asm__(".text\n"
        ".globl m36_result_wide\n"
        ".def m36_result_wide; .scl 2; .type 32; .endef\n"
        ".seh_proc m36_result_wide\n"
        "m36_result_wide:\n"
        ".seh_endprologue\n"
        "jmp m36_result_wide_storage\n"
        ".seh_endproc\n"
        ".globl m36_result_boxed\n"
        ".def m36_result_boxed; .scl 2; .type 32; .endef\n"
        ".seh_proc m36_result_boxed\n"
        "m36_result_boxed:\n"
        ".seh_endprologue\n"
        "jmp m36_result_boxed_storage\n"
        ".seh_endproc\n");
#else
#error "Scoop ABI fixture requires a supported target"
#endif
