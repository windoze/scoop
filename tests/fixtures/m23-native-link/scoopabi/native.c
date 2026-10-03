#include "scoop_rt.h"

#include <stddef.h>
#include <stdlib.h>
#include <string.h>

typedef struct Aggregate {
    const ScoopString *text;
    int64_t left;
    int64_t right;
} Aggregate;

_Static_assert(sizeof(Aggregate) == 24 && _Alignof(Aggregate) == 8 &&
               offsetof(Aggregate, text) == 0 && offsetof(Aggregate, left) == 8 &&
               offsetof(Aggregate, right) == 16, "Scoop value storage ABI");

static int32_t empty_calls;

void m23_empty(void) { empty_calls++; }
int32_t m23_zst_value(int32_t value) { return value + empty_calls; }

const ScoopString *m23_managed_echo(const ScoopString *text) {
    uintptr_t previous = (uintptr_t)text;
    void *root = (void *)text;
    void **slots[] = {&root};
    ScoopNativeRootFrame frame;
    scoop_rt_push_native_roots(&frame, slots, 1);
    scoop_runtime_gc_collect();
    const ScoopString *moved = root;
    if ((uintptr_t)moved == previous || moved->len != 13 ||
        memcmp(moved->data, "managed-alive", 13) != 0) {
        abort();
    }
    scoop_rt_pop_native_roots(&frame);
    return root;
}

void m23_aggregate_storage(Aggregate *result, Aggregate *value) {
    int64_t left = value->left;
    int64_t right = value->right;
    uintptr_t previous = (uintptr_t)value->text;
    void *root = (void *)value->text;
    void **slots[] = {&root};
    ScoopNativeRootFrame frame;
    scoop_rt_push_native_roots(&frame, slots, 1);
    scoop_runtime_gc_collect();
    if ((uintptr_t)root == previous) {
        abort();
    }
    result->text = root;
    result->left = left + 1;
    result->right = right + 2;
    previous = (uintptr_t)root;
    scoop_runtime_gc_collect();
    if ((uintptr_t)root == previous || result->text != root ||
        result->text->len != 13 || memcmp(result->text->data, "managed-alive", 13) != 0) {
        abort();
    }
    scoop_rt_pop_native_roots(&frame);
}

/* Scoop's AArch64 indirect result is in x8, while the 24-byte byval
 * argument is at the incoming stack pointer. This shim exposes both
 * storage locations to C without relying on Clang's aggregate classifier. */
__asm__(
    ".text\n"
    ".globl _m23_aggregate\n"
    ".p2align 2\n"
    "_m23_aggregate:\n"
    "mov x1, sp\n"
    "mov x0, x8\n"
    "b _m23_aggregate_storage\n");
