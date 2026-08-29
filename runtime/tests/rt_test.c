/* Runtime smoke test: provides scoop_main (normally emitted by the
 * compiler), exercises alloc, string concat/eq, and the String / Int /
 * Boolean print builtins, and checks that globals laid out the way
 * codegen emits string literals are readable by the runtime.
 *
 * Expected stdout (the M6 block prints no addresses, only booleans and
 * fixed values, so the output is deterministic):
 *   helloworld
 *   true
 *   false
 *   false
 *   42-7
 *   truefalse
 *   true
 *   true
 *   true
 *   true
 *   true
 *   true
 *   true
 *   false
 *   true
 *   7
 *   true
 *   false
 *   true
 *   true
 *
 * Build & run:
 *   cc -std=c11 -Wall -Wextra -I runtime/include runtime/src/rt.c runtime/tests/rt_test.c -o /tmp/scoop_rt_test
 *   /tmp/scoop_rt_test
 */
#include <string.h>

#include "scoop_rt.h"

/* Matches the layout of the @scoop_td_String global emitted by codegen.
 * Non-static: rt.c references it from scoop_rt_string_concat. */
const ScoopTypeDescriptor scoop_td_String = {1, 16, 8, NULL, NULL, NULL, NULL, 0};

/* Same layout codegen uses for StringConst globals:
 * { const ScoopTypeDescriptor *td; uint64_t len; char data[N]; }. */
typedef struct {
    const ScoopTypeDescriptor *td;
    uint64_t len;
    char data[5];
} FiveCharConst;

static const FiveCharConst hello = {&scoop_td_String, 5, {'h', 'e', 'l', 'l', 'o'}};
static const FiveCharConst world = {&scoop_td_String, 5, {'w', 'o', 'r', 'l', 'd'}};

/* M6 dispatch fixtures: interface Describable; class Shape; class
 * Point : Shape, Describable (vtable = Any slots + describe). */
static int64_t point_describe(const void *self) {
    (void)self;
    return 7;
}

static const ScoopTypeDescriptor describable_td = {1002, 0, 8, NULL, NULL, NULL, NULL, 0};
static const void *const point_describable_slots[] = {(const void *)&point_describe};
static const ScoopItableEntry point_itables[] = {{&describable_td, point_describable_slots}};
static const void *const point_vtable[] = {
    (const void *)&scoop_rt_any_equals,
    (const void *)&scoop_rt_any_hashcode,
    (const void *)&scoop_rt_any_tostring,
    (const void *)&point_describe,
};
static const ScoopTypeDescriptor shape_td = {1000, 16, 8, NULL, NULL, NULL, NULL, 0};
static const ScoopTypeDescriptor point_td = {
    1001, 24, 8, NULL, &shape_td, point_vtable, point_itables, 1};

void scoop_main(void) {
    const ScoopString *a = (const ScoopString *)&hello;
    const ScoopString *b = (const ScoopString *)&world;

    /* concat: "hello" + "world" -> "helloworld" */
    const ScoopString *concat = scoop_rt_string_concat(a, b);
    scoop_rt_println(concat);

    /* structural equality: same content, different content, length
     * mismatch */
    const ScoopString *copy = scoop_rt_string_concat(a, b);
    scoop_rt_println_boolean(scoop_rt_string_eq(concat, copy));
    scoop_rt_println_boolean(scoop_rt_string_eq(a, b));
    scoop_rt_println_boolean(scoop_rt_string_eq(a, concat));

    /* int / boolean output (print variants run into the println line) */
    scoop_rt_print_int(42);
    scoop_rt_println_int(-7);
    scoop_rt_print_boolean(true);
    scoop_rt_println_boolean(false);

    /* M7 primitive conversions backing core's intToString /
     * boolToString */
    scoop_rt_println(scoop_rt_int_to_string(42));
    scoop_rt_println(scoop_rt_int_to_string(-7));
    scoop_rt_println(scoop_rt_bool_to_string(true));
    scoop_rt_println(scoop_rt_bool_to_string(false));

    /* scoop_rt_array_clone (M5): independent snapshot — mutating the
     * original after the clone must not affect the copy, and the copy
     * keeps the header (td) and size. */
    const ScoopTypeDescriptor array_td = {100, 8, 8, NULL, NULL, NULL, NULL, 0};
    struct {
        const ScoopTypeDescriptor *td;
        uint64_t size;
        int64_t data[3];
    } original = {&array_td, 3, {10, 20, 30}};
    const ScoopArray *clone = scoop_rt_array_clone(&original, sizeof(int64_t));
    original.data[0] = 99;
    const int64_t *snapshot = (const int64_t *)clone->elements;
    scoop_rt_println_boolean(snapshot[0] == 10 && snapshot[1] == 20 && snapshot[2] == 30);
    scoop_rt_println_boolean(clone->size == 3 && clone->header.td == &array_td);

    /* scoop_rt_box (M6): header + payload copy. */
    int64_t payload[2] = {1, 2};
    const ScoopObjectHeader *boxed = scoop_rt_box(&point_td, payload, sizeof payload);
    scoop_rt_println_boolean(boxed->td == &point_td);
    const int64_t *boxed_payload = (const int64_t *)((const char *)boxed + sizeof(ScoopObjectHeader));
    scoop_rt_println_boolean(boxed_payload[0] == 1 && boxed_payload[1] == 2);

    /* scoop_rt_is_instance (M6): own td, parent chain, itable key;
     * unrelated td does not match. */
    scoop_rt_println_boolean(scoop_rt_is_instance(boxed, &point_td));
    scoop_rt_println_boolean(scoop_rt_is_instance(boxed, &shape_td));
    scoop_rt_println_boolean(scoop_rt_is_instance(boxed, &describable_td));
    scoop_rt_println_boolean(scoop_rt_is_instance(boxed, &scoop_td_String));

    /* scoop_rt_itable_lookup (M6): returns the recorded slots array. */
    const void *const *slots = scoop_rt_itable_lookup(&point_td, &describable_td);
    scoop_rt_println_boolean(slots == point_describable_slots);
    typedef int64_t (*DescribeFn)(const void *);
    scoop_rt_println_int(((DescribeFn)slots[0])(boxed));

    /* Any default methods (M6): identity equality, address hash,
     * "Object@<hex>" toString. */
    scoop_rt_println_boolean(scoop_rt_any_equals(boxed, boxed));
    scoop_rt_println_boolean(scoop_rt_any_equals(boxed, a));
    scoop_rt_println_boolean(scoop_rt_any_hashcode(boxed) == (uint64_t)(uintptr_t)boxed);
    const ScoopString *description = scoop_rt_any_tostring(boxed);
    scoop_rt_println_boolean(description->len > 7 && memcmp(description->data, "Object@", 7) == 0);

    /* scoop_rt_trap (M3) aborts the process, so it is not exercised
     * here; its trap path is covered end-to-end by EXPECT-TRAP compiler
     * fixtures. */
}
